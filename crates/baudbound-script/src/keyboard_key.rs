use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Reports why a key expression is not a valid `keyboard_key`, or `None` when
/// it is valid.
///
/// A keyboard key is an expression such as `Ctrl+S`, not a single key, and a
/// bare modifier such as `Ctrl` is valid on its own. The rule lives in this
/// crate so that the manifest validator, Script Settings and the runtime all
/// decide it the same way. It mirrors `validateWindowsKeyExpression` in the
/// editor, which is the contract both sides are written against.
pub fn keyboard_key_error(expression: &str) -> Option<String> {
    let parts: Vec<&str> = expression.split(['+', '-']).map(str::trim).collect();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Some("key expression must contain at least one supported key".to_owned());
    }

    let contract = key_contract_tokens();
    let mut seen = HashSet::new();
    for part in parts {
        let normalized = normalize_key_token(part);
        let Some(canonical) = contract
            .modifiers
            .get(&normalized)
            .or_else(|| contract.keys.get(&normalized))
        else {
            return Some(format!("{part} is not a known key"));
        };
        if !seen.insert(canonical.clone()) {
            return Some(format!(
                "key expression contains {canonical} more than once"
            ));
        }
    }

    None
}

/// Reports whether a key expression is a valid `keyboard_key`.
pub fn is_keyboard_key(expression: &str) -> bool {
    keyboard_key_error(expression).is_none()
}

fn normalize_key_token(token: &str) -> String {
    token
        .trim()
        .to_lowercase()
        .chars()
        .filter(|character| *character != ' ' && *character != '_')
        .collect()
}

/// Normalized modifier and key tokens mapped to their canonical names, kept as
/// two tables so a modifier always wins a collision, matching the editor.
struct KeyContractTokens {
    modifiers: HashMap<String, String>,
    keys: HashMap<String, String>,
}

fn key_contract_tokens() -> &'static KeyContractTokens {
    static TOKENS: OnceLock<KeyContractTokens> = OnceLock::new();
    TOKENS.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct KeyContract {
            modifiers: Vec<KeyEntry>,
            keys: Vec<KeyEntry>,
        }

        #[derive(serde::Deserialize)]
        struct KeyEntry {
            canonical: String,
            #[serde(default)]
            aliases: Vec<String>,
        }

        let contract: KeyContract = serde_json::from_str(include_str!(
            "../../../contracts/runner/windows-keyboard-keys.json"
        ))
        .expect("embedded keyboard key contract must be valid JSON");

        let build_table = |entries: Vec<KeyEntry>| {
            let mut table = HashMap::new();
            for entry in entries {
                table.insert(
                    normalize_key_token(&entry.canonical),
                    entry.canonical.clone(),
                );
                for alias in &entry.aliases {
                    table.insert(normalize_key_token(alias), entry.canonical.clone());
                }
            }
            table
        };

        KeyContractTokens {
            modifiers: build_table(contract.modifiers),
            keys: build_table(contract.keys),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_expressions_the_editor_accepts() {
        for expression in ["F5", "Ctrl+S", "control+s", "Ctrl", "Ctrl+Shift+F8"] {
            assert!(
                is_keyboard_key(expression),
                "{expression:?} should be a valid keyboard key"
            );
        }
        for expression in ["NotARealKey", "Ctrl+Ctrl", "Ctrl+", "", "Ctrl+control"] {
            assert!(
                !is_keyboard_key(expression),
                "{expression:?} should not be a valid keyboard key"
            );
        }
    }

    #[test]
    fn modifier_and_key_tokens_do_not_collide() {
        let contract = key_contract_tokens();
        let colliding: Vec<&String> = contract
            .modifiers
            .keys()
            .filter(|token| contract.keys.contains_key(*token))
            .collect();
        assert!(
            colliding.is_empty(),
            "modifier and key tokens must not collide, found: {colliding:?}"
        );
    }
}
