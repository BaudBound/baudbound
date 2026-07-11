use serde_json::{Value, json};

use crate::{
    CapabilityValidationError, validate_program_capabilities,
    validate_program_capabilities_with_secrets,
};

#[test]
fn validates_exact_capability_union_for_triggers_controls_and_actions() {
    let report = validate_program_capabilities(
        &program(
            &["trigger.webhook"],
            &["control.if", "action.http", "action.file.read"],
        ),
        &[
            "action.file",
            "action.http",
            "runtime.if",
            "trigger.manual",
            "trigger.webhook",
        ]
        .map(str::to_owned),
    )
    .expect("exact capability declaration should validate");

    assert_eq!(
        report
            .required_capabilities
            .iter()
            .map(|capability| capability.name.as_str())
            .collect::<Vec<_>>(),
        [
            "action.file",
            "action.http",
            "runtime.if",
            "trigger.manual",
            "trigger.webhook"
        ]
    );
}

#[test]
fn rejects_missing_extra_and_duplicate_capabilities() {
    let program = program(&[], &["action.http"]);
    let missing = validate_program_capabilities(&program, &["trigger.manual".to_owned()])
        .expect_err("missing action capability must fail");
    assert!(matches!(
        missing,
        CapabilityValidationError::MissingCapability(ref value) if value == "action.http"
    ));

    let extra = validate_program_capabilities(
        &program,
        &[
            "action.http".to_owned(),
            "action.file".to_owned(),
            "trigger.manual".to_owned(),
        ],
    )
    .expect_err("unused capability must fail");
    assert!(matches!(
        extra,
        CapabilityValidationError::UndeclaredCapability(ref value) if value == "action.file"
    ));

    let duplicate = validate_program_capabilities(
        &program,
        &[
            "action.http".to_owned(),
            "action.http".to_owned(),
            "trigger.manual".to_owned(),
        ],
    )
    .expect_err("duplicate capability must fail");
    assert!(matches!(
        duplicate,
        CapabilityValidationError::DuplicateCapability(ref value) if value == "action.http"
    ));
}

#[test]
fn rejects_unknown_action_types_and_invalid_program_shapes() {
    let unknown = validate_program_capabilities(
        &program(&[], &["action.hidden.unknown"]),
        &["trigger.manual".to_owned()],
    )
    .expect_err("unknown executable action must fail");
    assert!(matches!(
        unknown,
        CapabilityValidationError::UnsupportedActionType(ref value)
            if value == "action.hidden.unknown"
    ));

    let malformed = validate_program_capabilities(&json!({}), &[])
        .expect_err("invalid program shape must fail");
    assert!(matches!(
        malformed,
        CapabilityValidationError::InvalidProgram(_)
    ));
}

#[test]
fn derives_persistent_storage_and_secret_capabilities() {
    let mut program = program(&[], &["runtime.set_variable"]);
    program["entry"]["program"]["steps"][0]["config"]["scope"] = json!("persistent");
    let report = validate_program_capabilities_with_secrets(
        &program,
        &[
            "runtime.persistent_storage".to_owned(),
            "runtime.secrets".to_owned(),
            "runtime.variables".to_owned(),
            "trigger.manual".to_owned(),
        ],
        true,
    )
    .expect("scope-sensitive capabilities should validate");
    assert_eq!(report.required_capabilities.len(), 4);
}

fn program(secondary_triggers: &[&str], steps: &[&str]) -> Value {
    json!({
        "entry": {
            "trigger": node("n-manual", "trigger.manual"),
            "triggers": secondary_triggers
                .iter()
                .enumerate()
                .map(|(index, action_type)| node(&format!("n-trigger-{index}"), action_type))
                .collect::<Vec<_>>(),
            "program": {
                "steps": steps
                    .iter()
                    .enumerate()
                    .map(|(index, action_type)| node(&format!("n-step-{index}"), action_type))
                    .collect::<Vec<_>>(),
                "edges": []
            }
        }
    })
}

fn node(id: &str, action_type: &str) -> Value {
    json!({
        "id": id,
        "action_type": action_type,
        "config": if action_type == "runtime.set_variable" {
            json!({"scope": "runtime"})
        } else {
            json!({})
        },
        "runtime_outputs": []
    })
}
