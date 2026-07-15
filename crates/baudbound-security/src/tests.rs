use serde_json::json;

use super::*;

#[path = "tests/abuse_cases.rs"]
mod abuse_cases;
#[path = "tests/capabilities.rs"]
mod capabilities;

#[test]
fn validates_matching_permissions() {
    let report = validate_program_permissions(
        &program_with_steps(&["runtime.set_variable", "action.log", "action.file.read"]),
        &["file_read", "log", "set_local_variable"].map(str::to_owned),
        RiskLevel::Medium,
        &RunnerPolicy::default(),
    )
    .expect("permissions should validate");

    assert_eq!(report.calculated_risk, RiskLevel::Medium);
    assert_eq!(report.required_permissions.len(), 3);
}

#[test]
fn rejects_missing_permission() {
    let error = validate_program_permissions(
        &program_with_steps(&["action.file.read"]),
        &[],
        RiskLevel::Medium,
        &RunnerPolicy::default(),
    )
    .expect_err("missing permission should fail");

    assert!(error.to_string().contains("file_read"));
}

#[test]
fn rejects_stale_extra_permission() {
    let error = validate_program_permissions(
        &program_with_steps(&["action.log"]),
        &["log".to_owned(), "file_read".to_owned()],
        RiskLevel::Low,
        &RunnerPolicy::default(),
    )
    .expect_err("unused permission should fail");

    assert!(error.to_string().contains("file_read"));
}

#[test]
fn rejects_duplicate_permission() {
    let error = validate_program_permissions(
        &program_with_steps(&["action.log"]),
        &["log".to_owned(), "log".to_owned()],
        RiskLevel::Low,
        &RunnerPolicy::default(),
    )
    .expect_err("duplicate permission should fail");

    assert!(error.to_string().contains("duplicate permission log"));
}

#[test]
fn rejects_risk_mismatch() {
    let error = validate_program_permissions(
        &program_with_steps(&["action.file.read"]),
        &["file_read".to_owned()],
        RiskLevel::Low,
        &RunnerPolicy::default(),
    )
    .expect_err("wrong risk should fail");

    assert!(error.to_string().contains("risk_level"));
}

#[test]
fn policy_blocks_dangerous_permissions() {
    let error = validate_program_permissions(
        &program_with_steps(&["action.shell"]),
        &["run_shell_command".to_owned()],
        RiskLevel::Dangerous,
        &RunnerPolicy::default(),
    )
    .expect_err("dangerous action should be blocked");

    assert!(error.to_string().contains("dangerous actions are disabled"));
}

#[test]
fn derives_scope_and_secret_permissions_from_configuration() {
    let program = program_with_steps(&["runtime.set_variable"]);
    let mut program = program;
    program["entry"]["program"]["steps"][0]["config"]["scope"] = json!("global");
    let report = validate_program_permissions_with_secrets(
        &program,
        &["read_secret".to_owned(), "set_global_variable".to_owned()],
        RiskLevel::High,
        &RunnerPolicy::permissive(),
        true,
    )
    .expect("global and secret permissions should derive from package configuration");

    assert_eq!(
        report
            .required_permissions
            .iter()
            .map(|permission| permission.name.as_str())
            .collect::<Vec<_>>(),
        ["read_secret", "set_global_variable"]
    );
}

#[test]
fn rejects_legacy_writable_secret_scope() {
    let mut program = program_with_steps(&["runtime.set_variable"]);
    program["entry"]["program"]["steps"][0]["config"]["scope"] = json!("secret");
    let error = calculate_program_permissions(&program)
        .expect_err("secret scope must be read-only and declared in the manifest");
    assert!(error.to_string().contains("unsupported scope"));
}

#[test]
fn derives_permissions_from_manifest_default_variables() {
    let report = calculate_program_permissions_with_declarations(
        &program_with_steps(&[]),
        RuntimeDeclarationRequirements {
            has_persistent_default_variables: true,
            has_runtime_default_variables: true,
            has_secret_declarations: false,
        },
    )
    .expect("default variable permissions should derive from manifest requirements");

    assert_eq!(
        report
            .required_permissions
            .iter()
            .map(|permission| permission.name.as_str())
            .collect::<Vec<_>>(),
        ["set_local_variable", "set_persistent_variable"]
    );
    assert_eq!(report.calculated_risk, RiskLevel::Medium);
}

fn program_with_steps(action_types: &[&str]) -> Value {
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
                "steps": action_types
                    .iter()
                    .enumerate()
                    .map(|(index, action_type)| json!({
                        "id": format!("n-{index}"),
                        "action_type": action_type,
                        "type": "action",
                        "config": if *action_type == "runtime.set_variable" {
                            json!({"scope": "runtime"})
                        } else {
                            json!({})
                        },
                        "runtime_outputs": []
                    }))
                    .collect::<Vec<_>>(),
                "edges": []
            }
        }
    })
}
