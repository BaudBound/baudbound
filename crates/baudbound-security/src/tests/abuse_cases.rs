use crate::{
    PermissionValidationError, RiskLevel, RunnerPolicy, calculate_program_permissions,
    permission_for_action_type, validate_program_permissions,
};

use serde_json::json;

use super::program_with_steps;

#[test]
fn dangerous_actions_cannot_downgrade_their_declared_risk() {
    for (action_type, permission) in [
        ("action.shell", "run_shell_command"),
        ("action.file.delete", "delete_file"),
    ] {
        let error = validate_program_permissions(
            &program_with_steps(&[action_type]),
            &[permission.to_owned()],
            RiskLevel::Low,
            &RunnerPolicy::permissive(),
        )
        .expect_err("dangerous action with downgraded risk must fail");
        assert!(matches!(
            error,
            PermissionValidationError::RiskMismatch {
                expected: RiskLevel::Dangerous,
                ..
            }
        ));
    }
}

#[test]
fn shell_commands_have_an_independent_policy_gate() {
    let policy = RunnerPolicy {
        allow_dangerous_actions: true,
        allow_network_servers: true,
        allow_shell_commands: false,
    };
    let error = validate_program_permissions(
        &program_with_steps(&["action.shell"]),
        &["run_shell_command".to_owned()],
        RiskLevel::Dangerous,
        &policy,
    )
    .expect_err("shell-specific policy must block shell command");

    assert!(matches!(
        error,
        PermissionValidationError::PolicyBlocked { ref permission, .. }
            if permission == "run_shell_command"
    ));
}

#[test]
fn network_server_triggers_have_an_independent_policy_gate() {
    let policy = RunnerPolicy {
        allow_dangerous_actions: true,
        allow_network_servers: false,
        allow_shell_commands: true,
    };
    for (trigger, permission) in [
        ("trigger.webhook", "webhook_public_bind"),
        ("trigger.websocket", "websocket_public_bind"),
    ] {
        let error = validate_program_permissions(
            &program_with_steps(&[trigger]),
            &[permission.to_owned()],
            RiskLevel::High,
            &policy,
        )
        .expect_err("network-server policy must block public listener");
        assert!(matches!(
            error,
            PermissionValidationError::PolicyBlocked { permission: ref value, .. }
                if value == permission
        ));
    }
}

#[test]
fn process_kill_and_shell_permissions_keep_their_high_risk_classification() {
    let process_kill = permission_for_action_type("action.process.kill")
        .expect("process kill permission should exist");
    assert_eq!(process_kill.name, "process_kill");
    assert_eq!(process_kill.risk, RiskLevel::High);

    let shell =
        permission_for_action_type("action.shell").expect("shell command permission should exist");
    assert_eq!(shell.name, "run_shell_command");
    assert_eq!(shell.risk, RiskLevel::Dangerous);
}

#[test]
fn absolute_read_paths_cannot_use_the_limited_file_permission() {
    let mut program = program_with_steps(&["action.file.read"]);
    program["entry"]["program"]["steps"][0]["config"] =
        json!({"path": "C:\\Users\\user\\.ssh\\id_ed25519"});

    let error = validate_program_permissions(
        &program,
        &["file_read".to_owned()],
        RiskLevel::Medium,
        &RunnerPolicy::permissive(),
    )
    .expect_err("an absolute sensitive path must require the dangerous permission");

    assert!(matches!(
        error,
        PermissionValidationError::MissingPermission(ref permission)
            if permission == "read_sensitive_file"
    ));
}

#[test]
fn runtime_write_paths_require_unbounded_write_permission() {
    let mut program = program_with_steps(&["action.file.write"]);
    program["entry"]["program"]["steps"][0]["config"] =
        json!({"path": "{{trigger.body.destination}}"});

    let report = calculate_program_permissions(&program)
        .expect("runtime-derived write path should produce a permission report");

    assert_eq!(report.calculated_risk, RiskLevel::Dangerous);
    assert_eq!(
        report.required_permissions,
        [crate::PermissionGrant {
            name: "write_any_file".to_owned(),
            risk: RiskLevel::Dangerous,
        }]
    );
}

#[test]
fn repeated_file_actions_evaluate_every_node_configuration() {
    let mut program = program_with_steps(&["action.file.read", "action.file.read"]);
    program["entry"]["program"]["steps"][0]["config"] = json!({"path": "./input.txt"});
    program["entry"]["program"]["steps"][1]["config"] = json!({"path": "/etc/shadow"});

    let report = calculate_program_permissions(&program)
        .expect("every file action instance should be evaluated");
    let names = report
        .required_permissions
        .iter()
        .map(|permission| permission.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, ["file_read", "read_sensitive_file"]);
    assert_eq!(report.calculated_risk, RiskLevel::Dangerous);
}

#[test]
fn transfer_actions_keep_base_and_path_specific_permissions() {
    let mut program = program_with_steps(&["action.file.copy"]);
    program["entry"]["program"]["steps"][0]["config"] = json!({
        "sourcePath": "/etc/hosts",
        "destinationPath": "{{trigger.body.destination}}"
    });

    let report = calculate_program_permissions(&program)
        .expect("copy paths should derive independent source and destination permissions");
    let names = report
        .required_permissions
        .iter()
        .map(|permission| permission.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        ["file_copy", "read_sensitive_file", "write_any_file"]
    );
}
