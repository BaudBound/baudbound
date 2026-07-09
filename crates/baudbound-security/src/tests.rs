use serde_json::json;

use super::*;

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
                        "config": {},
                        "runtime_outputs": []
                    }))
                    .collect::<Vec<_>>(),
                "edges": []
            }
        }
    })
}
