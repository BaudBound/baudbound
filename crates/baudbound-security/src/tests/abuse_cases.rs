use crate::{
    PermissionValidationError, RiskLevel, RunnerPolicy, permission_for_action_type,
    validate_program_permissions,
};

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
