use serde_json::json;

use super::*;

#[path = "tests/abuse_cases.rs"]
mod abuse_cases;
#[path = "tests/capabilities.rs"]
mod capabilities;
#[path = "tests/filesystem_contract.rs"]
mod filesystem_contract;
#[path = "tests/path_classification.rs"]
mod path_classification;

#[test]
fn validates_matching_permissions() {
    let report = validate_program_permissions(
        &program_with_steps(&["runtime.set_variable", "action.log", "action.file.read"]),
        &["file.read", "log", "variable.local.set"].map(str::to_owned),
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

    assert!(error.to_string().contains("file.read"));
}

#[test]
fn rejects_stale_extra_permission() {
    let error = validate_program_permissions(
        &program_with_steps(&["action.log"]),
        &["log".to_owned(), "file.read".to_owned()],
        RiskLevel::Low,
        &RunnerPolicy::default(),
    )
    .expect_err("unused permission should fail");

    assert!(error.to_string().contains("file.read"));
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
        &["file.read".to_owned()],
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
        &["process.shell".to_owned()],
        RiskLevel::Dangerous,
        &RunnerPolicy::default(),
    )
    .expect_err("dangerous action should be blocked");

    assert!(error.to_string().contains("dangerous actions are disabled"));
}

#[test]
fn derives_scope_and_secret_permissions_from_configuration() {
    // The node names a variable; the declaration says the name is global. It
    // used to carry scope: "global" itself, which is the field that went away.
    let mut program = program_with_steps(&["runtime.set_variable"]);
    program["entry"]["program"]["steps"][0]["config"]["name"] = json!("shared_counter");
    let report = validate_program_permissions_with_declarations(
        &program,
        &["secret.read".to_owned(), "variable.global.set".to_owned()],
        RiskLevel::High,
        &RunnerPolicy::permissive(),
        RuntimeDeclarationRequirements {
            declared_variable_scopes: [("shared_counter".to_owned(), VariableScope::Global)]
                .into_iter()
                .collect(),
            has_global_declared_variables: true,
            has_secret_declarations: true,
            ..RuntimeDeclarationRequirements::default()
        },
    )
    .expect("global and secret permissions should derive from package configuration");

    assert_eq!(
        report
            .required_permissions
            .iter()
            .map(|permission| permission.name.as_str())
            .collect::<Vec<_>>(),
        ["secret.read", "variable.global.set"]
    );
}

#[test]
fn every_scope_derives_exactly_one_variable_write_and_never_reaches_a_secret() {
    // This replaces `rejects_legacy_writable_secret_scope`, which declared a
    // variable into scope "secret" and asserted the calculator refused it.
    // `VariableScope` has no secret variant, so that package now dies in
    // `baudbound-script` and the arm this exercised no longer exists.
    //
    // The invariant worth keeping is the one that test was reaching for: a
    // write derives a variable permission at the risk its scope carries, and
    // never anything that touches a secret. Pinning the whole table here means
    // a scope added later cannot quietly borrow another's risk — the match in
    // `calculate_program_permissions_with_declarations` stops compiling, and
    // this states what the new row has to say.
    let mut program = program_with_steps(&["runtime.set_variable"]);
    program["entry"]["program"]["steps"][0]["config"]["name"] = json!("api_token");

    for (scope, expected_permission, expected_risk) in [
        (VariableScope::Runtime, "variable.local.set", RiskLevel::Low),
        (
            VariableScope::Persistent,
            "variable.persistent.set",
            RiskLevel::Medium,
        ),
        (
            VariableScope::Global,
            "variable.global.set",
            RiskLevel::High,
        ),
    ] {
        let report = calculate_program_permissions_with_declarations(
            &program,
            RuntimeDeclarationRequirements {
                declared_variable_scopes: [("api_token".to_owned(), scope)].into_iter().collect(),
                // A secret is declared alongside, so the only secret permission
                // in the report has to be the read that declaration grants.
                has_secret_declarations: true,
                ..RuntimeDeclarationRequirements::default()
            },
        )
        .expect("every declarable scope derives a permission");

        let derived = report
            .required_permissions
            .iter()
            .map(|permission| (permission.name.as_str(), permission.risk))
            .collect::<Vec<_>>();

        // The calculator sorts by name, and "secret.read" precedes every
        // "variable.*", so the expected order is the same for all three.
        assert_eq!(
            derived,
            [
                ("secret.read", RiskLevel::High),
                (expected_permission, expected_risk),
            ],
            "{scope} derived the wrong permission set"
        );
    }
}

#[test]
fn a_variable_operation_carrying_no_scope_is_accepted() {
    // What the editor exports. The node names a declared variable and carries
    // no scope of its own; reading one off the node refused every such package
    // with "runtime.set_variable is missing string config.scope".
    let mut program = program_with_steps(&["runtime.set_variable"]);
    program["entry"]["program"]["steps"][0]["config"] = json!({
        "name": "counter",
        "operation": "increment",
        "value": "1"
    });
    let report = calculate_program_permissions_with_declarations(
        &program,
        RuntimeDeclarationRequirements {
            declared_variable_scopes: [("counter".to_owned(), VariableScope::Persistent)]
                .into_iter()
                .collect(),
            has_persistent_declared_variables: true,
            ..RuntimeDeclarationRequirements::default()
        },
    )
    .expect("a node without a scope must still derive its permission");

    assert!(
        report
            .required_permissions
            .iter()
            .any(|permission| permission.name == "variable.persistent.set"),
        "the declaration decides the permission, got {:?}",
        report.required_permissions
    );
}

#[test]
fn a_write_to_an_undeclared_name_asks_for_the_least_privilege() {
    // Nothing declares this name, so there is no scope to read. The run is
    // refused separately for writing an undeclared variable; asking for the
    // local permission here means that refusal is what the author sees rather
    // than a confusing permission mismatch.
    let mut program = program_with_steps(&["runtime.set_variable"]);
    program["entry"]["program"]["steps"][0]["config"]["name"] = json!("nothing_declares_this");
    let report = calculate_program_permissions_with_declarations(
        &program,
        RuntimeDeclarationRequirements::default(),
    )
    .expect("an undeclared write still derives a permission");

    assert!(
        report
            .required_permissions
            .iter()
            .any(|permission| permission.name == "variable.local.set"),
        "expected the least privileged variable permission, got {:?}",
        report.required_permissions
    );
}

#[test]
fn derives_permissions_from_manifest_declared_variables() {
    let report = calculate_program_permissions_with_declarations(
        &program_with_steps(&[]),
        RuntimeDeclarationRequirements {
            has_persistent_declared_variables: true,
            has_runtime_declared_variables: true,
            ..RuntimeDeclarationRequirements::default()
        },
    )
    .expect("declared variable permissions should derive from manifest requirements");

    assert_eq!(
        report
            .required_permissions
            .iter()
            .map(|permission| permission.name.as_str())
            .collect::<Vec<_>>(),
        ["variable.local.set", "variable.persistent.set"]
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
