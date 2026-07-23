use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::{
    Capability, RuntimeDeclarationRequirements, executable_action_types, first_duplicate,
    variable_operation_scopes,
};

const CAPABILITY_CONTRACT_VERSION: u32 = 1;
const CAPABILITY_CONTRACT_JSON: &str =
    include_str!("../../../contracts/runner/node-capabilities.json");

#[derive(Debug, Deserialize)]
struct NodeCapabilityContract {
    nodes: BTreeMap<String, Vec<String>>,
    version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramCapabilityReport {
    pub required_capabilities: Vec<Capability>,
}

#[derive(Debug, Error)]
pub enum CapabilityValidationError {
    #[error("program contains unsupported executable action type {0}")]
    UnsupportedActionType(String),
    #[error("capabilities.json is missing declared capability {0}")]
    MissingCapability(String),
    #[error("capabilities.json declares unused capability {0}")]
    UndeclaredCapability(String),
    #[error("capabilities.json declares duplicate capability {0}")]
    DuplicateCapability(String),
    #[error("program.json has invalid shape: {0}")]
    InvalidProgram(String),
    #[error("embedded node capability contract is invalid: {0}")]
    InvalidContract(String),
}

pub fn validate_program_capabilities(
    program: &Value,
    declared_capabilities: &[String],
) -> Result<ProgramCapabilityReport, CapabilityValidationError> {
    validate_program_capabilities_with_secrets(program, declared_capabilities, false)
}

pub fn validate_program_capabilities_with_secrets(
    program: &Value,
    declared_capabilities: &[String],
    has_secret_declarations: bool,
) -> Result<ProgramCapabilityReport, CapabilityValidationError> {
    validate_program_capabilities_with_declarations(
        program,
        declared_capabilities,
        RuntimeDeclarationRequirements {
            has_secret_declarations,
            ..RuntimeDeclarationRequirements::default()
        },
    )
}

pub fn validate_program_capabilities_with_declarations(
    program: &Value,
    declared_capabilities: &[String],
    requirements: RuntimeDeclarationRequirements,
) -> Result<ProgramCapabilityReport, CapabilityValidationError> {
    if let Some(duplicate) = first_duplicate(declared_capabilities) {
        return Err(CapabilityValidationError::DuplicateCapability(duplicate));
    }

    let report = calculate_program_capabilities_with_declarations(program, requirements)?;
    let required = report
        .required_capabilities
        .iter()
        .map(|capability| capability.name.as_str())
        .collect::<BTreeSet<_>>();
    let declared = declared_capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    for capability in &required {
        if !declared.contains(capability) {
            return Err(CapabilityValidationError::MissingCapability(
                (*capability).to_owned(),
            ));
        }
    }
    for capability in &declared {
        if !required.contains(capability) {
            return Err(CapabilityValidationError::UndeclaredCapability(
                (*capability).to_owned(),
            ));
        }
    }

    Ok(report)
}

pub fn calculate_program_capabilities(
    program: &Value,
) -> Result<ProgramCapabilityReport, CapabilityValidationError> {
    calculate_program_capabilities_with_secrets(program, false)
}

pub fn calculate_program_capabilities_with_secrets(
    program: &Value,
    has_secret_declarations: bool,
) -> Result<ProgramCapabilityReport, CapabilityValidationError> {
    calculate_program_capabilities_with_declarations(
        program,
        RuntimeDeclarationRequirements {
            has_secret_declarations,
            ..RuntimeDeclarationRequirements::default()
        },
    )
}

pub fn calculate_program_capabilities_with_declarations(
    program: &Value,
    requirements: RuntimeDeclarationRequirements,
) -> Result<ProgramCapabilityReport, CapabilityValidationError> {
    let contract = capability_contract()?;
    let mut names = BTreeSet::new();

    for action_type in
        executable_action_types(program).map_err(CapabilityValidationError::InvalidProgram)?
    {
        let capabilities = contract
            .nodes
            .get(&action_type)
            .ok_or_else(|| CapabilityValidationError::UnsupportedActionType(action_type.clone()))?;
        names.extend(capabilities.iter().cloned());
    }

    if variable_operation_scopes(program)
        .map_err(CapabilityValidationError::InvalidProgram)?
        .iter()
        .any(|scope| scope == "persistent" || scope == "global")
        || requirements.has_persistent_default_variables
    {
        names.insert("runtime.persistent_storage".to_owned());
    }
    if requirements.has_runtime_default_variables || requirements.has_persistent_default_variables {
        names.insert("runtime.variables".to_owned());
    }
    if requirements.has_secret_declarations {
        names.insert("runtime.secrets".to_owned());
    }

    Ok(ProgramCapabilityReport {
        required_capabilities: names.into_iter().map(|name| Capability { name }).collect(),
    })
}

fn capability_contract() -> Result<&'static NodeCapabilityContract, CapabilityValidationError> {
    static CONTRACT: OnceLock<Result<NodeCapabilityContract, String>> = OnceLock::new();
    match CONTRACT.get_or_init(parse_capability_contract) {
        Ok(contract) => Ok(contract),
        Err(message) => Err(CapabilityValidationError::InvalidContract(message.clone())),
    }
}

fn parse_capability_contract() -> Result<NodeCapabilityContract, String> {
    let contract = serde_json::from_str::<NodeCapabilityContract>(CAPABILITY_CONTRACT_JSON)
        .map_err(|source| source.to_string())?;
    if contract.version != CAPABILITY_CONTRACT_VERSION {
        return Err(format!(
            "unsupported version {}; expected {CAPABILITY_CONTRACT_VERSION}",
            contract.version
        ));
    }
    if contract.nodes.is_empty() {
        return Err("node mapping is empty".to_owned());
    }
    for (action_type, capabilities) in &contract.nodes {
        if action_type.trim().is_empty() || capabilities.is_empty() {
            return Err(format!(
                "node {action_type:?} must have a non-empty action type and capability list"
            ));
        }
        let unique = capabilities.iter().collect::<BTreeSet<_>>();
        if unique.len() != capabilities.len()
            || capabilities.iter().any(|value| value.trim().is_empty())
        {
            return Err(format!(
                "node {action_type:?} contains empty or duplicate capabilities"
            ));
        }
    }
    Ok(contract)
}
