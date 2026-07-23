use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
    time::SystemTime,
};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics,
    unix_timestamp_millis,
};

#[cfg(windows)]
mod native;
#[cfg(not(windows))]
mod native_unsupported;

#[cfg(windows)]
pub use native::NativeHotkeyService;
#[cfg(not(windows))]
pub use native_unsupported::NativeHotkeyService;

#[derive(Debug, Clone)]
pub struct HotkeyService {
    bindings: BTreeMap<String, Vec<TriggerRegistration>>,
}

impl HotkeyService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
    ) -> Result<Self, TriggerError> {
        let mut bindings = BTreeMap::<String, Vec<TriggerRegistration>>::new();
        for registration in registrations {
            if registration.action_type != "trigger.hotkey" {
                continue;
            }

            let hotkey = HotkeySpec::from_registration(&registration)?;
            bindings
                .entry(hotkey.normalized_key)
                .or_default()
                .push(registration);
        }

        Ok(Self { bindings })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.values().map(Vec::len).sum()
    }

    #[must_use]
    pub fn registered_hotkeys(&self) -> Vec<&str> {
        self.bindings.keys().map(String::as_str).collect()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        let hotkey_count = self.bindings.len();
        let registration_count = self.len();
        TriggerServiceDiagnostics {
            running: registration_count > 0,
            state: if registration_count > 0 {
                "active"
            } else {
                "idle"
            },
            summary: format!(
                "{registration_count} trigger(s) across {hotkey_count} hotkey binding(s)"
            ),
        }
    }

    pub fn events_for_key(
        &self,
        key: &str,
        timestamp: SystemTime,
    ) -> Result<Vec<TriggerEvent>, TriggerError> {
        let normalized_key = normalize_hotkey(key).map_err(|message| {
            TriggerError::Failed("trigger.hotkey".to_owned(), message.to_owned())
        })?;
        let Some(registrations) = self.bindings.get(&normalized_key) else {
            return Ok(Vec::new());
        };
        let timestamp = unix_timestamp_millis(timestamp).to_string();

        Ok(registrations
            .iter()
            .map(|registration| TriggerEvent {
                action_type: registration.action_type.clone(),
                node_id: registration.node_id.clone(),
                payload: json!({
                    "key": normalized_key,
                    "timestamp": timestamp,
                }),
                script_id: registration.script_id.clone(),
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HotkeySpec {
    normalized_key: String,
}

impl HotkeySpec {
    pub(crate) fn from_registration(
        registration: &TriggerRegistration,
    ) -> Result<Self, TriggerError> {
        let key = registration
            .config
            .get("key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "hotkey trigger must define key".to_owned(),
                )
            })?;
        if key.contains("{{") || key.contains("}}") {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "hotkey trigger key cannot use runtime variable templates".to_owned(),
            ));
        }

        Ok(Self {
            normalized_key: normalize_hotkey(key)
                .map_err(|message| TriggerError::Failed(registration.node_id.clone(), message))?,
        })
    }
}

fn normalize_hotkey(input: &str) -> Result<String, String> {
    parse_hotkey(input).map(|hotkey| hotkey.expression)
}

pub(crate) const MODIFIER_CTRL: u8 = 1;
pub(crate) const MODIFIER_ALT: u8 = 2;
pub(crate) const MODIFIER_SHIFT: u8 = 4;
pub(crate) const MODIFIER_WINDOWS: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedHotkey {
    pub(crate) expression: String,
    pub(crate) modifiers: u8,
    pub(crate) virtual_keys: Vec<u32>,
}

pub(crate) fn parse_hotkey(input: &str) -> Result<ParsedHotkey, String> {
    let catalog = hotkey_catalog()?;
    let parts = input.split(['+', '-']).map(str::trim).collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(format!(
            "hotkey {input:?} must contain at least one supported key"
        ));
    }
    let mut modifiers = 0;
    let mut selected_keys = BTreeMap::<String, u32>::new();

    for part in parts {
        let token = normalize_contract_token(part);
        if let Some(modifier) = catalog.modifiers_by_alias.get(&token) {
            if modifiers & modifier.mask != 0 {
                return Err(format!(
                    "hotkey {input:?} contains modifier {:?} more than once",
                    modifier.canonical
                ));
            }
            modifiers |= modifier.mask;
            continue;
        }
        let key = catalog.keys_by_alias.get(&token).ok_or_else(|| {
            format!("hotkey key {part:?} is not supported by the Windows key contract")
        })?;
        if selected_keys
            .insert(key.canonical.clone(), key.virtual_key)
            .is_some()
        {
            return Err(format!(
                "hotkey {input:?} contains key {:?} more than once",
                key.canonical
            ));
        }
    }

    let mut expression = Vec::new();
    for modifier in &catalog.modifiers {
        if modifiers & modifier.mask != 0 {
            expression.push(modifier.canonical.clone());
        }
    }
    expression.extend(selected_keys.keys().cloned());

    let mut virtual_keys = selected_keys.into_values().collect::<Vec<_>>();
    virtual_keys.sort_unstable();

    Ok(ParsedHotkey {
        expression: expression.join("+"),
        modifiers,
        virtual_keys,
    })
}

#[cfg(windows)]
pub(crate) fn modifier_virtual_keys() -> Result<&'static BTreeMap<u32, u8>, String> {
    hotkey_catalog().map(|catalog| &catalog.modifier_virtual_keys)
}

#[derive(Debug, Deserialize)]
struct HotkeyContract {
    version: u32,
    modifiers: Vec<ContractModifier>,
    keys: Vec<ContractKey>,
}

#[derive(Debug, Deserialize)]
struct ContractModifier {
    canonical: String,
    aliases: Vec<String>,
    virtual_keys: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct ContractKey {
    canonical: String,
    aliases: Vec<String>,
    virtual_key: u32,
}

#[derive(Debug)]
struct CatalogModifier {
    canonical: String,
    mask: u8,
}

#[derive(Debug)]
struct HotkeyCatalog {
    modifiers: Vec<CatalogModifier>,
    modifiers_by_alias: BTreeMap<String, CatalogModifier>,
    #[cfg(windows)]
    modifier_virtual_keys: BTreeMap<u32, u8>,
    keys_by_alias: BTreeMap<String, ContractKey>,
}

fn hotkey_catalog() -> Result<&'static HotkeyCatalog, String> {
    static CATALOG: OnceLock<Result<HotkeyCatalog, String>> = OnceLock::new();
    CATALOG
        .get_or_init(build_hotkey_catalog)
        .as_ref()
        .map_err(Clone::clone)
}

fn build_hotkey_catalog() -> Result<HotkeyCatalog, String> {
    let contract: HotkeyContract = serde_json::from_str(include_str!(
        "../../../../contracts/runner/windows-keyboard-keys.json"
    ))
    .map_err(|error| format!("Windows hotkey key contract is invalid: {error}"))?;
    if contract.version != 1 {
        return Err(format!(
            "Windows hotkey key contract version {} is not supported",
            contract.version
        ));
    }

    if contract.modifiers.len() != 4 {
        return Err("Windows hotkey key contract must define four modifiers".to_owned());
    }
    let mut modifiers = Vec::new();
    let mut modifiers_by_alias = BTreeMap::new();
    let mut modifier_virtual_keys = BTreeMap::new();
    let mut declared_modifier_masks = BTreeSet::new();
    for modifier in contract.modifiers {
        let mask = match modifier.canonical.as_str() {
            "Ctrl" => MODIFIER_CTRL,
            "Alt" => MODIFIER_ALT,
            "Shift" => MODIFIER_SHIFT,
            "Windows" => MODIFIER_WINDOWS,
            unsupported => {
                return Err(format!(
                    "Windows hotkey contract contains unsupported modifier {unsupported:?}"
                ));
            }
        };
        if !declared_modifier_masks.insert(mask) {
            return Err(format!(
                "Windows hotkey contract contains duplicate modifier {:?}",
                modifier.canonical
            ));
        }
        let catalog_modifier = CatalogModifier {
            canonical: modifier.canonical.clone(),
            mask,
        };
        for alias in std::iter::once(&modifier.canonical).chain(&modifier.aliases) {
            insert_unique(
                &mut modifiers_by_alias,
                normalize_contract_token(alias),
                CatalogModifier {
                    canonical: modifier.canonical.clone(),
                    mask,
                },
                "modifier alias",
            )?;
        }
        for virtual_key in modifier.virtual_keys {
            insert_unique(
                &mut modifier_virtual_keys,
                virtual_key,
                mask,
                "modifier virtual key",
            )?;
        }
        modifiers.push(catalog_modifier);
    }
    modifiers.sort_by_key(|modifier| modifier.mask);

    let mut keys_by_alias = BTreeMap::new();
    for key in contract.keys {
        let aliases = std::iter::once(&key.canonical)
            .chain(&key.aliases)
            .map(|alias| normalize_contract_token(alias))
            .collect::<BTreeSet<_>>();
        for alias in aliases {
            insert_unique(
                &mut keys_by_alias,
                alias,
                ContractKey {
                    canonical: key.canonical.clone(),
                    aliases: Vec::new(),
                    virtual_key: key.virtual_key,
                },
                "key alias",
            )?;
        }
    }

    Ok(HotkeyCatalog {
        modifiers,
        modifiers_by_alias,
        #[cfg(windows)]
        modifier_virtual_keys,
        keys_by_alias,
    })
}

fn insert_unique<K: Ord, V>(
    values: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    label: &str,
) -> Result<(), String> {
    if values.insert(key, value).is_some() {
        return Err(format!(
            "Windows hotkey contract contains a duplicate {label}"
        ));
    }
    Ok(())
}

fn normalize_contract_token(input: &str) -> String {
    input
        .trim()
        .chars()
        .filter(|character| !matches!(character, ' ' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}
