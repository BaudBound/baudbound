//! Values the runner tells a script about the machine and the moment.
//!
//! These are read once per run rather than per reference, so every reference
//! inside one run agrees. A run that took a fresh clock reading each time
//! could log one minute at the top of a script and the next further down.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// Builds the `system_` variables for one run.
#[must_use]
pub fn system_variables() -> BTreeMap<String, Value> {
    let now = chrono::Local::now();
    BTreeMap::from([
        ("system_os".to_owned(), json!(operating_system())),
        (
            "system_arch".to_owned(),
            json!(std::env::consts::ARCH.to_owned()),
        ),
        ("system_hostname".to_owned(), json!(host_name())),
        ("system_user".to_owned(), json!(user_name())),
        ("system_locale".to_owned(), json!(locale())),
        ("system_timezone".to_owned(), json!(timezone())),
        (
            // A datetime rather than preformatted text, so a script can read a
            // part of it or render it however it wants.
            "system_datetime".to_owned(),
            json!({ "type": "datetime", "value": now.to_rfc3339() }),
        ),
    ])
}

/// The vocabulary the editor already shows, rather than the Rust target name.
fn operating_system() -> String {
    match std::env::consts::OS {
        "windows" => "windows".to_owned(),
        "linux" => "linux".to_owned(),
        other => other.to_owned(),
    }
}

fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_owned())
}

fn user_name() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_owned())
}

/// Best effort. There is no portable way to read the operating system's
/// locale without another dependency, so this reads the usual environment
/// variables and falls back to a fixed value rather than reporting something
/// it did not actually find.
fn locale() -> String {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name) {
            let tag = value
                .split('.')
                .next()
                .unwrap_or_default()
                .replace('_', "-");
            if !tag.is_empty() && tag != "C" && tag != "POSIX" {
                return tag;
            }
        }
    }
    "en-US".to_owned()
}

fn timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_system_variable_is_supplied() {
        // The editor offers these in its variable picker. Anything it lists
        // and the runner does not supply reaches production as literal braces,
        // which is what every one of them used to do.
        let variables = system_variables();
        for name in [
            "system_os",
            "system_arch",
            "system_hostname",
            "system_user",
            "system_locale",
            "system_timezone",
            "system_datetime",
        ] {
            assert!(variables.contains_key(name), "{name} must be supplied");
        }
        assert_eq!(
            variables.len(),
            7,
            "an unlisted system variable would be invisible to the editor"
        );
    }

    #[test]
    fn the_datetime_is_a_datetime_value_the_runtime_accepts() {
        let variables = system_variables();
        let datetime = &variables["system_datetime"];

        assert_eq!(datetime["type"], "datetime");
        let value = datetime["value"].as_str().expect("value should be text");
        assert!(
            chrono::DateTime::parse_from_rfc3339(value).is_ok(),
            "the value must parse as RFC 3339: {value}"
        );
    }

    #[test]
    fn text_values_are_never_empty() {
        // An empty string reads as a missing value to an author, so each
        // lookup falls back rather than reporting nothing.
        let variables = system_variables();
        for (name, value) in &variables {
            if name == "system_datetime" {
                continue;
            }
            let text = value.as_str().unwrap_or_default();
            assert!(!text.is_empty(), "{name} should never be empty");
        }
    }
}

#[cfg(test)]
mod runtime_integration_tests {
    use baudbound_runtime::{
        RuntimeExecutionResources, UnsupportedActionHandler, execute_manual_program_with_state,
    };
    use serde_json::json;

    use super::system_variables;

    #[test]
    fn a_run_resolves_a_system_reference_instead_of_printing_the_braces() {
        // Before these were supplied, {{system_os}} reached production as the
        // literal text "{{system_os}}" with an empty variables map.
        let supplied = system_variables();
        let program = json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {},
                    "runtime_outputs": []
                },
                "triggers": [],
                "program": { "steps": [], "edges": [] }
            }
        });

        let report = execute_manual_program_with_state(
            &program,
            "script-1",
            RuntimeExecutionResources::new(&UnsupportedActionHandler)
                .with_system_variables(&supplied),
        )
        .expect("the run should finish");

        assert_eq!(
            report.variables.get("system_os"),
            supplied.get("system_os"),
            "a system value must reach the run"
        );
        assert!(
            report.variables.contains_key("system_datetime"),
            "the datetime must reach the run"
        );
        assert_eq!(
            report
                .variable_scopes
                .get("system_os")
                .map(|scope| scope.as_str()),
            Some("system"),
            "the Variables panel should report where the value came from"
        );
    }
}
