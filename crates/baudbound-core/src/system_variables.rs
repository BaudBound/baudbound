//! What the runner tells a script about the machine it is running on.
//!
//! These are the fields of the `@system` object. The `@` is what makes them
//! safe: no user identifier may contain one, so a script cannot shadow a
//! built-in and the runner does not have to reserve any name to protect them.
//!
//! Every field here describes the machine and cannot change during a run, so
//! each is read once when the run starts. Readings that do change — the clock,
//! the uptime — are supplied by the runtime at reference time instead, because
//! a run can loop or wait for hours and a value read at the top of it would be
//! wrong everywhere else.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// The machine fields of `@system`, read once per run.
#[must_use]
pub fn system_variables() -> BTreeMap<String, Value> {
    let mut system = sysinfo::System::new();
    system.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());

    BTreeMap::from([
        ("os".to_owned(), json!(operating_system())),
        ("os_name".to_owned(), json!(os_name())),
        ("os_version".to_owned(), json!(os_version())),
        ("arch".to_owned(), json!(std::env::consts::ARCH.to_owned())),
        ("hostname".to_owned(), json!(host_name())),
        ("user".to_owned(), json!(user_name())),
        ("locale".to_owned(), json!(locale())),
        ("timezone".to_owned(), json!(timezone())),
        ("cpu_count".to_owned(), json!(system.cpus().len())),
        ("runner_version".to_owned(), json!(runner_version())),
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

fn os_name() -> String {
    sysinfo::System::long_os_version().unwrap_or_else(operating_system)
}

fn os_version() -> String {
    sysinfo::System::kernel_version()
        .or_else(sysinfo::System::os_version)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_owned())
}

fn user_name() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_owned())
}

/// The runner that is actually running, which is not the same fact as the
/// minimum version a package declares it needs.
fn runner_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
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

/// The manifest fields a run exposes as `@manifest`.
///
/// These were never supplied to a run at all. The editor offered
/// `manifest_name` and the rest, typed them, and resolved them in simulation,
/// while a real run printed the braces, exactly as the system values did.
///
/// `version` is the script version the manifest declares. The editor used to
/// report the manifest format version here, which is a different fact and not
/// one an author has any use for.
///
/// `format_version`, `script_language_version` and `created_with` are
/// deliberately absent: they describe how the package was written rather than
/// what the script is, and an author branching on them would be reading the
/// toolchain rather than their own script. The declaration collections
/// (`assets`, `variables`, `settings`, `secrets`) are absent for a different
/// reason — each is already reachable as a variable in its own right, so
/// copying them here would give one value two spellings that could disagree.
#[must_use]
pub fn manifest_variables(manifest: &baudbound_script::Manifest) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("id".to_owned(), json!(manifest.id.clone())),
        ("name".to_owned(), json!(manifest.name.clone())),
        ("version".to_owned(), json!(manifest.version.clone())),
        ("author".to_owned(), json!(manifest.author.clone())),
        (
            "description".to_owned(),
            json!(manifest.description.clone()),
        ),
        ("website".to_owned(), json!(manifest.website.clone())),
        ("source".to_owned(), json!(manifest.source.clone())),
        (
            "minimum_runner_version".to_owned(),
            json!(manifest.minimum_runner_version.clone()),
        ),
        // `repository_url` is deliberately absent. It is distribution plumbing
        // rather than something a script has any business branching on, and
        // `source` already answers where the script came from. The editor's
        // package-contract test holds the same line on its side.
        //
        // A datetime rather than the raw string, so the component paths and the
        // format patterns read it the same way they read any other datetime.
        (
            "created_at".to_owned(),
            json!({"type": "datetime", "value": manifest.created_at.clone()}),
        ),
        ("tags".to_owned(), json!(manifest.tags.clone())),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_machine_field_the_editor_offers_is_supplied() {
        // The editor lists these in its variable picker. Anything it offers and
        // the runner does not supply reaches production as literal braces,
        // which is what every system value used to do.
        let fields = system_variables();
        for name in [
            "os",
            "os_name",
            "os_version",
            "arch",
            "hostname",
            "user",
            "locale",
            "timezone",
            "cpu_count",
            "runner_version",
        ] {
            assert!(fields.contains_key(name), "@system.{name} must be supplied");
        }
        assert_eq!(
            fields.len(),
            10,
            "an unlisted field would be invisible to the editor"
        );
    }

    #[test]
    fn no_field_here_can_change_during_a_run() {
        // The clock and the uptime are deliberately absent: they are read at
        // reference time by the runtime. A reading that changes has no business
        // in a map that is built once.
        let fields = system_variables();
        assert!(!fields.contains_key("datetime"));
        assert!(!fields.contains_key("uptime"));
    }

    #[test]
    fn text_values_are_never_empty() {
        // An empty string reads as a missing value to an author, so each
        // lookup falls back rather than reporting nothing.
        for (name, value) in &system_variables() {
            if let Some(text) = value.as_str() {
                assert!(!text.is_empty(), "{name} should never be empty");
            }
        }
    }

    #[test]
    fn the_cpu_count_is_a_positive_integer() {
        let fields = system_variables();
        let cpus = fields["cpu_count"].as_u64().expect("cpu_count is a number");
        assert!(cpus > 0, "a machine running this has at least one CPU");
    }
}

#[cfg(test)]
mod runtime_integration_tests {
    use baudbound_runtime::{
        RuntimeExecutionResources, UnsupportedActionHandler, execute_manual_program_with_state,
    };
    use serde_json::json;

    use super::{manifest_variables, system_variables};

    fn empty_program() -> serde_json::Value {
        json!({
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
        })
    }

    #[test]
    fn a_run_resolves_system_fields_instead_of_printing_the_braces() {
        // Before these were supplied, {{system_os}} reached production as the
        // literal text "{{system_os}}" with an empty variables map.
        let supplied = system_variables();
        let report = execute_manual_program_with_state(
            &empty_program(),
            "script-1",
            RuntimeExecutionResources::new(&UnsupportedActionHandler)
                .with_system_variables(&supplied),
        )
        .expect("the run should finish");

        let system = report
            .variables
            .get("@system")
            .expect("@system must reach the run");
        assert_eq!(system.get("os"), supplied.get("os"));
        assert_eq!(
            report
                .variable_scopes
                .get("@system")
                .map(|scope| scope.as_str()),
            Some("system"),
            "the Variables panel should report where the value came from"
        );
    }

    #[test]
    fn a_run_is_told_about_itself() {
        let report = execute_manual_program_with_state(
            &empty_program(),
            "script-1",
            RuntimeExecutionResources::new(&UnsupportedActionHandler),
        )
        .expect("the run should finish");

        let system = report.variables.get("@system").expect("@system exists");
        assert_eq!(
            system.get("run_id").and_then(|value| value.as_str()),
            Some(report.identity.run_id.as_str())
        );
        assert_eq!(
            system.get("trigger_id").and_then(|value| value.as_str()),
            Some("n-trigger")
        );
        assert_eq!(
            system.get("trigger_type").and_then(|value| value.as_str()),
            Some("trigger.manual")
        );
        assert_eq!(
            system
                .get("run_started_at")
                .and_then(|value| value.get("type")),
            Some(&json!("datetime"))
        );
    }

    #[test]
    fn a_run_resolves_manifest_fields_that_never_reached_it_before() {
        // The editor has always offered these. The runner supplied none of
        // them, so every one printed its own braces in production.
        let manifest = baudbound_script::Manifest {
            format_version: 1,
            script_language_version: 1,
            id: "script-1".to_owned(),
            name: "Probe".to_owned(),
            description: String::new(),
            author: "NATroutter".to_owned(),
            website: String::new(),
            source: String::new(),
            created_with: "test".to_owned(),
            created_at: String::new(),
            updated_at: String::new(),
            tags: Vec::new(),
            minimum_runner_version: "2.0.0".to_owned(),
            version: "1.4.2".to_owned(),
            repository_url: String::new(),
            assets: Vec::new(),
            variables: Vec::new(),
            settings: Vec::new(),
            secrets: Vec::new(),
        };
        let supplied = manifest_variables(&manifest);
        let report = execute_manual_program_with_state(
            &empty_program(),
            "script-1",
            RuntimeExecutionResources::new(&UnsupportedActionHandler)
                .with_manifest_variables(&supplied),
        )
        .expect("the run should finish");

        let resolved = report
            .variables
            .get("@manifest")
            .expect("@manifest must reach the run");
        assert_eq!(resolved.get("name"), Some(&json!("Probe")));
        // The script version, not the manifest format version the editor used
        // to report under this name.
        assert_eq!(resolved.get("version"), Some(&json!("1.4.2")));
        assert_eq!(
            report
                .variable_scopes
                .get("@manifest")
                .map(|scope| scope.as_str()),
            Some("manifest")
        );
    }
}

#[cfg(test)]
mod live_field_tests {
    use baudbound_runtime::{
        RuntimeExecutionResources, UnsupportedActionHandler, execute_manual_program_with_state,
    };
    use serde_json::json;

    /// Two log nodes with a delay between them, which is the shape the bug was
    /// reported in: a script that waits and then reads the clock again.
    fn program() -> serde_json::Value {
        json!({
            "entry": {
                "trigger": {
                    "id": "n-trigger",
                    "action_type": "trigger.manual",
                    "type": "manual",
                    "config": {},
                    "runtime_outputs": []
                },
                "triggers": [],
                "program": {
                    "steps": [
                        {
                            "id": "n-first",
                            "action_type": "action.log",
                            "type": "log",
                            "config": {
                                "level": "info",
                                "message": "first={{@system.datetime.value}} started={{@system.run_started_at.value}}"
                            },
                            "runtime_outputs": []
                        },
                        {
                            "id": "n-wait",
                            "action_type": "action.delay",
                            "type": "delay",
                            "config": { "amount": "1100", "unit": "milliseconds" },
                            "runtime_outputs": []
                        },
                        {
                            "id": "n-second",
                            "action_type": "action.log",
                            "type": "log",
                            "config": {
                                "level": "info",
                                "message": "second={{@system.datetime.value}} started={{@system.run_started_at.value}}"
                            },
                            "runtime_outputs": []
                        }
                    ],
                    "edges": [
                        {"execution_order": 0, "source": "n-trigger", "source_handle": "out", "target": "n-first", "target_handle": "input"},
                        {"execution_order": 0, "source": "n-first", "source_handle": "success", "target": "n-wait", "target_handle": "input"},
                        {"execution_order": 0, "source": "n-wait", "source_handle": "success", "target": "n-second", "target_handle": "input"}
                    ]
                }
            }
        })
    }

    fn field(message: &str, key: &str) -> String {
        message
            .split_whitespace()
            .find_map(|part| part.strip_prefix(key))
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn the_clock_moves_between_nodes_while_the_run_start_does_not() {
        let report = execute_manual_program_with_state(
            &program(),
            "script-1",
            RuntimeExecutionResources::new(&UnsupportedActionHandler),
        )
        .expect("the run should finish");

        let logged = |prefix: &str| {
            report
                .logs
                .iter()
                .find(|entry| entry.message.starts_with(prefix))
                .map(|entry| entry.message.clone())
                .unwrap_or_default()
        };
        let first = logged("first=");
        let second = logged("second=");
        assert!(!first.is_empty() && !second.is_empty(), "both logs exist");

        // The reported bug: read once per run, these two were identical no
        // matter how long the script waited between them.
        assert_ne!(
            field(&first, "first="),
            field(&second, "second="),
            "the clock must move across a delay: {first} / {second}"
        );

        // And the stable counterpart, which is what "every reference agrees"
        // was really asking for.
        assert_eq!(
            field(&first, "started="),
            field(&second, "started="),
            "run_started_at must not move"
        );
    }

    #[test]
    fn two_references_in_one_field_agree() {
        // The boundary is a node execution, so a single field cannot straddle a
        // tick however many times it names the clock.
        let mut program = program();
        program["entry"]["program"]["steps"][0]["config"]["message"] =
            json!("a={{@system.datetime.value}} b={{@system.datetime.value}}");

        let report = execute_manual_program_with_state(
            &program,
            "script-1",
            RuntimeExecutionResources::new(&UnsupportedActionHandler),
        )
        .expect("the run should finish");

        let message = report
            .logs
            .iter()
            .find(|entry| entry.message.starts_with("a="))
            .map(|entry| entry.message.clone())
            .expect("the log exists");
        assert_eq!(field(&message, "a="), field(&message, "b="), "{message}");
    }
}
