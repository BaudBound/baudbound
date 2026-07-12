use std::{collections::BTreeMap, time::SystemTime};

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
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    let mut primary_key = None::<String>;

    for part in input
        .split(['+', '-'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match normalized_hotkey_token(part).as_str() {
            "Ctrl" => ctrl = true,
            "Alt" => alt = true,
            "Shift" => shift = true,
            "Meta" => meta = true,
            token if primary_key.is_none() => primary_key = Some(token.to_owned()),
            token => {
                return Err(format!(
                    "hotkey {input:?} contains multiple primary keys ({:?} and {token:?})",
                    primary_key.unwrap_or_default()
                ));
            }
        }
    }

    let primary_key =
        primary_key.ok_or_else(|| format!("hotkey {input:?} must include a primary key"))?;
    if !is_supported_primary_key(&primary_key) {
        return Err(format!(
            "hotkey key {primary_key:?} is not supported; use A-Z, 0-9, F1-F24, or a supported navigation key"
        ));
    }
    let mut parts = Vec::new();
    if ctrl {
        parts.push("Ctrl".to_owned());
    }
    if alt {
        parts.push("Alt".to_owned());
    }
    if shift {
        parts.push("Shift".to_owned());
    }
    if meta {
        parts.push("Meta".to_owned());
    }
    parts.push(primary_key);

    Ok(parts.join("+"))
}

fn is_supported_primary_key(key: &str) -> bool {
    if key.len() == 1 {
        let byte = key.as_bytes()[0];
        return byte.is_ascii_uppercase() || byte.is_ascii_digit();
    }

    if let Some(number) = key
        .strip_prefix('F')
        .and_then(|value| value.parse::<u8>().ok())
    {
        return (1..=24).contains(&number);
    }

    matches!(
        key,
        "Escape"
            | "Enter"
            | "Space"
            | "Tab"
            | "Backspace"
            | "Delete"
            | "Insert"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
    )
}

fn normalized_hotkey_token(input: &str) -> String {
    match input.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => "Ctrl".to_owned(),
        "alt" | "option" => "Alt".to_owned(),
        "shift" => "Shift".to_owned(),
        "meta" | "cmd" | "command" | "win" | "windows" | "super" => "Meta".to_owned(),
        "esc" | "escape" => "Escape".to_owned(),
        "return" | "enter" => "Enter".to_owned(),
        "space" | "spacebar" => "Space".to_owned(),
        "tab" => "Tab".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "delete" | "del" => "Delete".to_owned(),
        "insert" | "ins" => "Insert".to_owned(),
        "home" => "Home".to_owned(),
        "end" => "End".to_owned(),
        "pageup" | "page_up" | "page up" => "PageUp".to_owned(),
        "pagedown" | "page_down" | "page down" => "PageDown".to_owned(),
        "up" | "arrowup" | "arrow up" => "ArrowUp".to_owned(),
        "down" | "arrowdown" | "arrow down" => "ArrowDown".to_owned(),
        "left" | "arrowleft" | "arrow left" => "ArrowLeft".to_owned(),
        "right" | "arrowright" | "arrow right" => "ArrowRight".to_owned(),
        token if token.len() == 1 => token.to_ascii_uppercase(),
        token => {
            let mut chars = token.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!("{}{}", first.to_uppercase(), chars.as_str())
        }
    }
}
