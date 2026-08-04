//! Guards the permission contract against filesystem-reaching config drift.
//!
//! A node that reads a path from its config and hands it to the filesystem must
//! declare a matching `path_rules` entry, otherwise the path is never classified
//! and an absolute or templated value cannot escalate to a Dangerous permission.

use super::*;

/// Config keys that reach the filesystem, paired with the node that reads them.
///
/// This list is maintained by hand on purpose. Reflection over the action
/// implementations would let a new filesystem key appear without anyone
/// deciding whether it needs classification. Requiring an edit here forces that
/// decision at the point the key is added.
const FILESYSTEM_CONFIG_KEYS: &[(&str, &str)] = &[
    ("action.file.copy", "sourcePath"),
    ("action.file.copy", "destinationPath"),
    ("action.file.delete", "path"),
    ("action.file.download", "destinationPath"),
    ("action.file.move", "sourcePath"),
    ("action.file.move", "destinationPath"),
    ("action.file.read", "path"),
    ("action.file.write", "path"),
    ("action.sound.play", "filePath"),
    ("trigger.file_watch", "path"),
];

#[test]
fn every_filesystem_config_key_has_a_contract_path_rule() {
    let contract = permission_contract().expect("permission contract should parse");
    let mut missing = Vec::new();

    for (action_type, config_key) in FILESYSTEM_CONFIG_KEYS {
        let definition = contract
            .nodes
            .get(*action_type)
            .unwrap_or_else(|| panic!("permission contract is missing node {action_type}"));
        if !definition
            .path_rules
            .iter()
            .any(|rule| rule.config_key == *config_key)
        {
            missing.push(format!("{action_type} reads {config_key}"));
        }
    }

    assert!(
        missing.is_empty(),
        "these config keys reach the filesystem but have no path rule, so their paths are never \
         classified and cannot escalate to a Dangerous permission: {}",
        missing.join("; ")
    );
}

#[test]
fn contract_path_rules_are_covered_by_the_filesystem_key_list() {
    let contract = permission_contract().expect("permission contract should parse");
    let mut unlisted = Vec::new();

    for (action_type, definition) in &contract.nodes {
        for rule in &definition.path_rules {
            let listed = FILESYSTEM_CONFIG_KEYS
                .iter()
                .any(|(node, key)| node == action_type && *key == rule.config_key);
            if !listed {
                unlisted.push(format!("{action_type} declares {}", rule.config_key));
            }
        }
    }

    assert!(
        unlisted.is_empty(),
        "the contract declares path rules that the filesystem key list does not know about, so the \
         two have drifted apart: {}",
        unlisted.join("; ")
    );
}
