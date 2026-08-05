use crate::{
    PermissionValidationError, RiskLevel, RunnerPolicy, calculate_program_permissions,
    permission_for_action_type, validate_program_permissions,
};

use serde_json::json;

use super::program_with_steps;

#[test]
fn dangerous_actions_cannot_downgrade_their_declared_risk() {
    for (action_type, permission) in [
        ("action.shell", "process.shell"),
        ("action.process.run", "process.run"),
        ("action.application.open", "process.run"),
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
        allow_shell_commands: false,
    };
    let error = validate_program_permissions(
        &program_with_steps(&["action.shell"]),
        &["process.shell".to_owned()],
        RiskLevel::Dangerous,
        &policy,
    )
    .expect_err("shell-specific policy must block shell command");

    assert!(matches!(
        error,
        PermissionValidationError::PolicyBlocked { ref permission, .. }
            if permission == "process.shell"
    ));
}

#[test]
fn network_triggers_are_package_capabilities_not_public_bind_permissions() {
    let policy = RunnerPolicy {
        allow_dangerous_actions: true,
        allow_shell_commands: true,
    };
    for (trigger, permission) in [
        ("trigger.webhook", "network.webhook"),
        ("trigger.websocket", "network.websocket"),
    ] {
        validate_program_permissions(
            &program_with_steps(&[trigger]),
            &[permission.to_owned()],
            RiskLevel::High,
            &policy,
        )
        .expect("public listener policy is enforced at listener startup, not package validation");
    }
}

#[test]
fn process_execution_permissions_keep_their_security_classification() {
    let process_kill = permission_for_action_type("action.process.kill")
        .expect("process kill permission should exist");
    assert_eq!(process_kill.name, "process.kill");
    assert_eq!(process_kill.risk, RiskLevel::High);

    let run_process = permission_for_action_type("action.process.run")
        .expect("run process permission should exist");
    assert_eq!(run_process.name, "process.run");
    assert_eq!(run_process.risk, RiskLevel::Dangerous);

    let open_application = permission_for_action_type("action.application.open")
        .expect("open application permission should exist");
    assert_eq!(open_application.name, "process.run");
    assert_eq!(open_application.risk, RiskLevel::Dangerous);

    let shell =
        permission_for_action_type("action.shell").expect("shell command permission should exist");
    assert_eq!(shell.name, "process.shell");
    assert_eq!(shell.risk, RiskLevel::Dangerous);
}

#[test]
fn dangerous_action_policy_blocks_run_process() {
    let program = program_with_steps(&["action.process.run"]);

    let error = validate_program_permissions(
        &program,
        &["process.run".to_owned()],
        RiskLevel::Dangerous,
        &RunnerPolicy {
            allow_dangerous_actions: false,
            ..RunnerPolicy::permissive()
        },
    )
    .expect_err("dangerous-action policy must block process execution");

    assert!(matches!(
        error,
        PermissionValidationError::PolicyBlocked { ref permission, .. }
            if permission == "process.run"
    ));
}

#[test]
fn absolute_read_paths_cannot_use_the_limited_file_permission() {
    let mut program = program_with_steps(&["action.file.read"]);
    program["entry"]["program"]["steps"][0]["config"] =
        json!({"path": "C:\\Users\\user\\.ssh\\id_ed25519"});

    let error = validate_program_permissions(
        &program,
        &["file.read".to_owned()],
        RiskLevel::Medium,
        &RunnerPolicy::permissive(),
    )
    .expect_err("an absolute sensitive path must require the dangerous permission");

    assert!(matches!(
        error,
        PermissionValidationError::MissingPermission(ref permission)
            if permission == "file.read.any"
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
            name: "file.write.any".to_owned(),
            risk: RiskLevel::Dangerous,
        }]
    );
}

#[test]
fn parent_traversal_paths_require_unbounded_permissions() {
    for path in [
        "../outside.txt",
        "nested/../../outside.txt",
        r"..\outside.txt",
    ] {
        let mut program = program_with_steps(&["action.file.write"]);
        program["entry"]["program"]["steps"][0]["config"] = json!({"path": path});

        let report = calculate_program_permissions(&program)
            .expect("parent traversal should produce a permission report");

        assert!(report.required_permissions.iter().any(|permission| {
            permission.name == "file.write.any" && permission.risk == RiskLevel::Dangerous
        }));
        assert_eq!(report.calculated_risk, RiskLevel::Dangerous);
    }
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

    assert_eq!(names, ["file.read", "file.read.any"]);
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

    assert_eq!(names, ["file.copy", "file.read.any", "file.write.any"]);
}

#[test]
fn observation_triggers_derive_permissions_from_their_effective_scope() {
    let mut limited_watch = program_with_steps(&["trigger.file_watch"]);
    limited_watch["entry"]["program"]["steps"][0]["config"] = json!({"path": "workspace/incoming"});
    let limited_report = calculate_program_permissions(&limited_watch)
        .expect("a workspace-relative watch should derive a limited observation permission");
    assert_eq!(limited_report.calculated_risk, RiskLevel::Medium);
    assert_eq!(
        limited_report.required_permissions[0].name,
        "file.watch.limited"
    );

    for path in [
        "C:\\Users\\operator\\Documents",
        "/srv/incoming",
        "{{settings.watchPath}}",
    ] {
        let mut unbounded_watch = program_with_steps(&["trigger.file_watch"]);
        unbounded_watch["entry"]["program"]["steps"][0]["config"] = json!({"path": path});
        let report = calculate_program_permissions(&unbounded_watch)
            .expect("an arbitrary watch should derive the host-wide observation permission");
        assert_eq!(report.calculated_risk, RiskLevel::Dangerous, "{path}");
        assert_eq!(
            report.required_permissions[0].name, "file.watch.any",
            "{path}"
        );
    }

    let process_report =
        calculate_program_permissions(&program_with_steps(&["trigger.process_started"]))
            .expect("process observation should require explicit approval");
    assert_eq!(process_report.calculated_risk, RiskLevel::Medium);
    assert_eq!(
        process_report.required_permissions[0].name,
        "process.observe"
    );
}
