use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use enigo::Key;
use serde::Deserialize;

#[derive(Debug)]
pub(super) struct ParsedKeyCombo {
    pub(super) expression: String,
    pub(super) canonical_keys: Vec<String>,
    pub(super) keys: Vec<Key>,
}

pub(super) fn parse_key_combo(input: &str) -> Result<ParsedKeyCombo, String> {
    let parts = input.split(['+', '-']).map(str::trim).collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err("key expression must contain at least one supported key".to_owned());
    }

    let catalog = keyboard_catalog()?;
    let mut selected_modifiers = BTreeSet::new();
    let mut selected_keys = BTreeSet::new();
    for token in parts {
        let normalized = normalize_token(token);
        if let Some(canonical) = catalog.modifier_aliases.get(&normalized) {
            if selected_modifiers.insert(canonical.as_str()) {
                continue;
            }
            return Err(format!(
                "key expression contains the {canonical} modifier more than once"
            ));
        }
        let canonical = catalog
            .key_aliases
            .get(&normalized)
            .ok_or_else(|| format!("key {token:?} is not supported by the Windows key contract"))?;
        if !selected_keys.insert(canonical.as_str()) {
            return Err(format!(
                "key expression contains {canonical} more than once"
            ));
        }
    }

    let mut expression_parts = Vec::new();
    let mut keys = Vec::new();
    for (canonical, key) in &catalog.modifier_order {
        if selected_modifiers.contains(canonical.as_str()) {
            expression_parts.push(canonical.clone());
            keys.push(*key);
        }
    }
    for canonical in selected_keys {
        let key = native_primary_key(canonical).ok_or_else(|| {
            format!("Windows key contract entry {canonical:?} has no native mapping")
        })?;
        expression_parts.push(canonical.to_owned());
        keys.push(key);
    }

    Ok(ParsedKeyCombo {
        expression: expression_parts.join("+"),
        canonical_keys: expression_parts,
        keys,
    })
}

#[derive(Debug, Deserialize)]
struct WindowsKeyboardContract {
    version: u32,
    modifiers: Vec<ContractEntry>,
    keys: Vec<ContractEntry>,
}

#[derive(Debug, Deserialize)]
struct ContractEntry {
    canonical: String,
    aliases: Vec<String>,
}

#[derive(Debug)]
struct KeyboardCatalog {
    modifier_order: Vec<(String, Key)>,
    modifier_aliases: BTreeMap<String, String>,
    key_aliases: BTreeMap<String, String>,
}

fn keyboard_catalog() -> Result<&'static KeyboardCatalog, String> {
    static CATALOG: OnceLock<Result<KeyboardCatalog, String>> = OnceLock::new();
    CATALOG
        .get_or_init(build_keyboard_catalog)
        .as_ref()
        .map_err(Clone::clone)
}

fn build_keyboard_catalog() -> Result<KeyboardCatalog, String> {
    let contract: WindowsKeyboardContract = serde_json::from_str(include_str!(
        "../../../../../crates/baudbound-script/contracts/windows-keyboard-keys.json"
    ))
    .map_err(|error| format!("Windows keyboard contract is invalid: {error}"))?;
    if contract.version != 1 {
        return Err(format!(
            "Windows keyboard contract version {} is not supported",
            contract.version
        ));
    }

    let mut modifier_order = Vec::new();
    let mut modifier_aliases = BTreeMap::new();
    for modifier in contract.modifiers {
        let key = native_modifier_key(&modifier.canonical).ok_or_else(|| {
            format!(
                "Windows keyboard contract modifier {:?} has no native mapping",
                modifier.canonical
            )
        })?;
        insert_aliases(
            &mut modifier_aliases,
            &modifier.canonical,
            &modifier.aliases,
            "modifier",
        )?;
        modifier_order.push((modifier.canonical, key));
    }

    let mut key_aliases = BTreeMap::new();
    for key in contract.keys {
        if native_primary_key(&key.canonical).is_none() {
            return Err(format!(
                "Windows keyboard contract key {:?} has no native mapping",
                key.canonical
            ));
        }
        insert_aliases(&mut key_aliases, &key.canonical, &key.aliases, "key")?;
    }

    Ok(KeyboardCatalog {
        modifier_order,
        modifier_aliases,
        key_aliases,
    })
}

fn insert_aliases(
    values: &mut BTreeMap<String, String>,
    canonical: &str,
    aliases: &[String],
    kind: &str,
) -> Result<(), String> {
    for alias in std::iter::once(canonical).chain(aliases.iter().map(String::as_str)) {
        if let Some(previous) = values.insert(normalize_token(alias), canonical.to_owned())
            && previous != canonical
        {
            return Err(format!(
                "Windows keyboard contract {kind} alias {alias:?} is shared by {previous:?} and {canonical:?}"
            ));
        }
    }
    Ok(())
}

fn native_modifier_key(canonical: &str) -> Option<Key> {
    match canonical {
        "Ctrl" => Some(Key::Control),
        "Alt" => Some(Key::Alt),
        "Shift" => Some(Key::Shift),
        "Windows" => Some(Key::LWin),
        _ => None,
    }
}

#[allow(clippy::too_many_lines)]
fn native_primary_key(canonical: &str) -> Option<Key> {
    Some(match canonical {
        "A" => Key::A,
        "B" => Key::B,
        "C" => Key::C,
        "D" => Key::D,
        "E" => Key::E,
        "F" => Key::F,
        "G" => Key::G,
        "H" => Key::H,
        "I" => Key::I,
        "J" => Key::J,
        "K" => Key::K,
        "L" => Key::L,
        "M" => Key::M,
        "N" => Key::N,
        "O" => Key::O,
        "P" => Key::P,
        "Q" => Key::Q,
        "R" => Key::R,
        "S" => Key::S,
        "T" => Key::T,
        "U" => Key::U,
        "V" => Key::V,
        "W" => Key::W,
        "X" => Key::X,
        "Y" => Key::Y,
        "Z" => Key::Z,
        "0" => Key::Num0,
        "1" => Key::Num1,
        "2" => Key::Num2,
        "3" => Key::Num3,
        "4" => Key::Num4,
        "5" => Key::Num5,
        "6" => Key::Num6,
        "7" => Key::Num7,
        "8" => Key::Num8,
        "9" => Key::Num9,
        "F1" => Key::F1,
        "F2" => Key::F2,
        "F3" => Key::F3,
        "F4" => Key::F4,
        "F5" => Key::F5,
        "F6" => Key::F6,
        "F7" => Key::F7,
        "F8" => Key::F8,
        "F9" => Key::F9,
        "F10" => Key::F10,
        "F11" => Key::F11,
        "F12" => Key::F12,
        "F13" => Key::F13,
        "F14" => Key::F14,
        "F15" => Key::F15,
        "F16" => Key::F16,
        "F17" => Key::F17,
        "F18" => Key::F18,
        "F19" => Key::F19,
        "F20" => Key::F20,
        "F21" => Key::F21,
        "F22" => Key::F22,
        "F23" => Key::F23,
        "F24" => Key::F24,
        "Escape" => Key::Escape,
        "Enter" => Key::Return,
        "Space" => Key::Space,
        "Tab" => Key::Tab,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Insert" => Key::Insert,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "ArrowUp" => Key::UpArrow,
        "ArrowDown" => Key::DownArrow,
        "ArrowLeft" => Key::LeftArrow,
        "ArrowRight" => Key::RightArrow,
        "CapsLock" => Key::CapsLock,
        "NumLock" => Key::Numlock,
        "ScrollLock" => Key::Scroll,
        "PrintScreen" => Key::PrintScr,
        "Pause" => Key::Pause,
        "ContextMenu" => Key::Apps,
        "Semicolon" => Key::OEM1,
        "Equal" => Key::OEMPlus,
        "Comma" => Key::OEMComma,
        "Minus" => Key::OEMMinus,
        "Period" => Key::OEMPeriod,
        "Slash" => Key::OEM2,
        "Backquote" => Key::OEM3,
        "BracketLeft" => Key::OEM4,
        "Backslash" => Key::OEM5,
        "BracketRight" => Key::OEM6,
        "Quote" => Key::OEM7,
        "IntlBackslash" => Key::OEM102,
        "Numpad0" => Key::Numpad0,
        "Numpad1" => Key::Numpad1,
        "Numpad2" => Key::Numpad2,
        "Numpad3" => Key::Numpad3,
        "Numpad4" => Key::Numpad4,
        "Numpad5" => Key::Numpad5,
        "Numpad6" => Key::Numpad6,
        "Numpad7" => Key::Numpad7,
        "Numpad8" => Key::Numpad8,
        "Numpad9" => Key::Numpad9,
        "NumpadMultiply" => Key::Multiply,
        "NumpadAdd" => Key::Add,
        "NumpadSeparator" => Key::Separator,
        "NumpadSubtract" => Key::Subtract,
        "NumpadDecimal" => Key::Decimal,
        "NumpadDivide" => Key::Divide,
        "BrowserBack" => Key::BrowserBack,
        "BrowserForward" => Key::BrowserForward,
        "BrowserRefresh" => Key::BrowserRefresh,
        "BrowserStop" => Key::BrowserStop,
        "BrowserSearch" => Key::BrowserSearch,
        "BrowserFavorites" => Key::BrowserFavorites,
        "BrowserHome" => Key::BrowserHome,
        "VolumeMute" => Key::VolumeMute,
        "VolumeDown" => Key::VolumeDown,
        "VolumeUp" => Key::VolumeUp,
        "MediaNext" => Key::MediaNextTrack,
        "MediaPrevious" => Key::MediaPrevTrack,
        "MediaStop" => Key::MediaStop,
        "MediaPlayPause" => Key::MediaPlayPause,
        "LaunchMail" => Key::LaunchMail,
        "LaunchMedia" => Key::LaunchMediaSelect,
        "LaunchApp1" => Key::LaunchApp1,
        "LaunchApp2" => Key::LaunchApp2,
        _ => return None,
    })
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| !matches!(character, ' ' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_canonicalizes_supported_combinations() {
        let combo = parse_key_combo("control + shift + media play pause + F1")
            .expect("declared aliases should parse");

        assert_eq!(combo.expression, "Ctrl+Shift+F1+MediaPlayPause");
        assert_eq!(
            combo.keys,
            [Key::Control, Key::Shift, Key::F1, Key::MediaPlayPause]
        );
    }

    #[test]
    fn maps_every_declared_contract_entry_to_a_native_key() {
        let catalog = keyboard_catalog().expect("the generated contract should be valid");

        assert_eq!(catalog.modifier_order.len(), 4);
        assert!(!catalog.key_aliases.is_empty());
    }

    #[test]
    fn rejects_unknown_keys_instead_of_typing_their_first_character() {
        let error = parse_key_combo("Ctrl+NotARealKey").expect_err("unknown key must fail");

        assert!(error.contains("not supported"));
    }

    #[test]
    fn supports_multiple_non_modifier_keys_and_rejects_duplicates() {
        assert_eq!(
            parse_key_combo("K+L")
                .expect("multi-key chord should parse")
                .keys,
            [Key::K, Key::L]
        );
        assert_eq!(
            parse_key_combo("F1+T")
                .expect("function-key chord should parse")
                .keys,
            [Key::F1, Key::T]
        );
        assert!(parse_key_combo("Ctrl+Control+B").is_err());
        assert!(parse_key_combo("A+A").is_err());
    }
}
