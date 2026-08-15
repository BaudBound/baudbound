//! Security and policy primitives shared by BaudBound runner apps.

mod blacklist;
mod capabilities;
mod network;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub use baudbound_script::VariableScope;
pub use blacklist::{
    BlacklistDecision, BlacklistEntry, BlacklistMatchSubject, BlacklistPolicy, BlacklistScope,
    BlacklistSeverity, PermissiveBlacklistPolicy, github_publisher_for_url, matching_entries,
    normalize_blacklist_entry, normalize_repository_url,
};
pub use capabilities::{
    CapabilityValidationError, ProgramCapabilityReport, calculate_program_capabilities,
    calculate_program_capabilities_with_declarations, calculate_program_capabilities_with_secrets,
    validate_program_capabilities, validate_program_capabilities_with_declarations,
    validate_program_capabilities_with_secrets,
};
pub use network::{
    all_network_addresses_are_public, is_https_downgrade, is_public_network_address,
    same_http_origin,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDeclarationRequirements {
    /// The scope each declared variable was declared with, by name.
    ///
    /// A Variable Operation node no longer carries a scope: it names a declared
    /// variable and the declaration settles it. Deriving a permission therefore
    /// means resolving the node's target through this map, which is what the
    /// editor does when it writes permissions.json. Reading the node instead
    /// left the runner asking a question the program could no longer answer.
    pub declared_variable_scopes: BTreeMap<String, VariableScope>,
    pub has_global_declared_variables: bool,
    pub has_persistent_declared_variables: bool,
    pub has_runtime_declared_variables: bool,
    pub has_secret_declarations: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Dangerous,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PermissionGrant {
    pub name: String,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Capability {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunnerPolicy {
    pub allow_dangerous_actions: bool,
    pub allow_shell_commands: bool,
}

impl RunnerPolicy {
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allow_dangerous_actions: true,
            allow_shell_commands: true,
        }
    }
}

#[must_use]
pub fn highest_risk(permissions: &[PermissionGrant]) -> RiskLevel {
    permissions
        .iter()
        .map(|permission| permission.risk)
        .max()
        .unwrap_or(RiskLevel::Low)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPermissionReport {
    pub required_permissions: Vec<PermissionGrant>,
    pub calculated_risk: RiskLevel,
}

#[derive(Debug, Error)]
pub enum PermissionValidationError {
    #[error("program contains unsupported executable action type {0}")]
    UnsupportedActionType(String),
    #[error("permissions.json is missing declared permission {0}")]
    MissingPermission(String),
    #[error("permissions.json declares unused permission {0}")]
    UndeclaredPermission(String),
    #[error("permissions.json declares duplicate permission {0}")]
    DuplicatePermission(String),
    #[error("permissions.json risk_level is {declared:?}, expected {expected:?}")]
    RiskMismatch {
        declared: RiskLevel,
        expected: RiskLevel,
    },
    #[error("runner policy blocks permission {permission}: {reason}")]
    PolicyBlocked { permission: String, reason: String },
    #[error("program.json has invalid shape: {0}")]
    InvalidProgram(String),
    #[error("embedded node permission contract is invalid: {0}")]
    InvalidContract(String),
}

const PERMISSION_CONTRACT_VERSION: u32 = 1;
const PERMISSION_CONTRACT_JSON: &str =
    include_str!("../../../contracts/runner/node-permissions.json");

#[derive(Debug, Deserialize)]
struct NodePermissionContract {
    nodes: BTreeMap<String, NodePermissionDefinition>,
    version: u32,
}

#[derive(Debug, Deserialize)]
struct NodePermissionDefinition {
    permission: Option<PermissionGrant>,
    path_rules: Vec<PathPermissionRule>,
}

#[derive(Debug, Deserialize)]
struct PathPermissionRule {
    access: PathAccess,
    config_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PathAccess {
    Delete,
    Read,
    Watch,
    Write,
}

#[derive(Debug, Error)]
pub enum SecurityValidationError {
    #[error(transparent)]
    Capability(#[from] CapabilityValidationError),
    #[error(transparent)]
    Permission(#[from] PermissionValidationError),
}

pub fn validate_program_permissions(
    program: &Value,
    declared_permissions: &[String],
    declared_risk: RiskLevel,
    policy: &RunnerPolicy,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    validate_program_permissions_with_secrets(
        program,
        declared_permissions,
        declared_risk,
        policy,
        false,
    )
}

pub fn validate_program_permissions_with_secrets(
    program: &Value,
    declared_permissions: &[String],
    declared_risk: RiskLevel,
    policy: &RunnerPolicy,
    has_secret_declarations: bool,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    validate_program_permissions_with_declarations(
        program,
        declared_permissions,
        declared_risk,
        policy,
        RuntimeDeclarationRequirements {
            has_secret_declarations,
            ..RuntimeDeclarationRequirements::default()
        },
    )
}

pub fn validate_program_permissions_with_declarations(
    program: &Value,
    declared_permissions: &[String],
    declared_risk: RiskLevel,
    policy: &RunnerPolicy,
    requirements: RuntimeDeclarationRequirements,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    let report = calculate_program_permissions_with_declarations(program, requirements)?;
    let required = report
        .required_permissions
        .iter()
        .map(|permission| permission.name.as_str())
        .collect::<BTreeSet<_>>();
    let declared = declared_permissions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    if declared.len() != declared_permissions.len() {
        let duplicate = first_duplicate(declared_permissions)
            .expect("set length proves that a duplicate permission exists");
        return Err(PermissionValidationError::DuplicatePermission(duplicate));
    }

    for permission in &required {
        if !declared.contains(permission) {
            return Err(PermissionValidationError::MissingPermission(
                (*permission).to_owned(),
            ));
        }
    }

    for permission in &declared {
        if !required.contains(permission) {
            return Err(PermissionValidationError::UndeclaredPermission(
                (*permission).to_owned(),
            ));
        }
    }

    if declared_risk != report.calculated_risk {
        return Err(PermissionValidationError::RiskMismatch {
            declared: declared_risk,
            expected: report.calculated_risk,
        });
    }

    enforce_runner_policy(&report.required_permissions, policy)?;
    Ok(report)
}

pub fn calculate_program_permissions(
    program: &Value,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    calculate_program_permissions_with_secrets(program, false)
}

pub fn calculate_program_permissions_with_secrets(
    program: &Value,
    has_secret_declarations: bool,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    calculate_program_permissions_with_declarations(
        program,
        RuntimeDeclarationRequirements {
            has_secret_declarations,
            ..RuntimeDeclarationRequirements::default()
        },
    )
}

pub fn calculate_program_permissions_with_declarations(
    program: &Value,
    requirements: RuntimeDeclarationRequirements,
) -> Result<ProgramPermissionReport, PermissionValidationError> {
    let mut permissions = Vec::<PermissionGrant>::new();
    let mut seen_permissions = BTreeSet::<String>::new();

    let contract = permission_contract()?;
    for (action_type, config) in
        executable_nodes(program).map_err(PermissionValidationError::InvalidProgram)?
    {
        if action_type == "runtime.set_variable" {
            continue;
        }
        let definition = contract
            .nodes
            .get(&action_type)
            .ok_or_else(|| PermissionValidationError::UnsupportedActionType(action_type.clone()))?;

        let replaces_base_permission = definition.path_rules.iter().any(|rule| {
            matches!(
                (
                    &rule.access,
                    definition
                        .permission
                        .as_ref()
                        .map(|value| value.name.as_str())
                ),
                (PathAccess::Delete, Some("file.delete.any"))
                    | (PathAccess::Read, Some("file.read"))
                    | (PathAccess::Watch, Some("file.watch.limited"))
                    | (PathAccess::Write, Some("file.write.limited"))
            )
        });
        if let Some(permission) = &definition.permission
            && !replaces_base_permission
        {
            insert_permission(&mut permissions, &mut seen_permissions, permission.clone());
        }
        for rule in &definition.path_rules {
            let path = config
                .get(&rule.config_key)
                .map(config_value_string)
                .unwrap_or_default();
            insert_permission(
                &mut permissions,
                &mut seen_permissions,
                permission_for_path(&rule.access, &path),
            );
        }
    }

    for scope in variable_operation_scopes(program, &requirements.declared_variable_scopes)
        .map_err(PermissionValidationError::InvalidProgram)?
    {
        // Exhaustive over the scopes that exist. There is no arm for an
        // unrecognised one because `VariableScope` cannot hold one, which is
        // what stops a new scope from silently inheriting a permission: adding
        // a variant breaks this match until someone states its risk.
        let permission = match scope {
            VariableScope::Runtime => PermissionGrant {
                name: "variable.local.set".to_owned(),
                risk: RiskLevel::Low,
            },
            VariableScope::Persistent => PermissionGrant {
                name: "variable.persistent.set".to_owned(),
                risk: RiskLevel::Medium,
            },
            VariableScope::Global => PermissionGrant {
                name: "variable.global.set".to_owned(),
                risk: RiskLevel::High,
            },
        };
        if seen_permissions.insert(permission.name.clone()) {
            permissions.push(permission);
        }
    }

    if requirements.has_runtime_declared_variables {
        insert_permission(
            &mut permissions,
            &mut seen_permissions,
            PermissionGrant {
                name: "variable.local.set".to_owned(),
                risk: RiskLevel::Low,
            },
        );
    }
    if requirements.has_persistent_declared_variables {
        insert_permission(
            &mut permissions,
            &mut seen_permissions,
            PermissionGrant {
                name: "variable.persistent.set".to_owned(),
                risk: RiskLevel::Medium,
            },
        );
    }
    if requirements.has_secret_declarations && seen_permissions.insert("secret.read".to_owned()) {
        permissions.push(PermissionGrant {
            name: "secret.read".to_owned(),
            risk: RiskLevel::High,
        });
    }

    permissions.sort_by(|left, right| left.name.cmp(&right.name));
    let calculated_risk = highest_risk(&permissions);

    Ok(ProgramPermissionReport {
        required_permissions: permissions,
        calculated_risk,
    })
}

pub fn permission_for_action_type(action_type: &str) -> Option<PermissionGrant> {
    permission_contract()
        .ok()?
        .nodes
        .get(action_type)?
        .permission
        .clone()
}

fn permission_contract() -> Result<&'static NodePermissionContract, PermissionValidationError> {
    static CONTRACT: OnceLock<Result<NodePermissionContract, String>> = OnceLock::new();
    match CONTRACT.get_or_init(parse_permission_contract) {
        Ok(contract) => Ok(contract),
        Err(message) => Err(PermissionValidationError::InvalidContract(message.clone())),
    }
}

fn parse_permission_contract() -> Result<NodePermissionContract, String> {
    let contract = serde_json::from_str::<NodePermissionContract>(PERMISSION_CONTRACT_JSON)
        .map_err(|source| source.to_string())?;
    if contract.version != PERMISSION_CONTRACT_VERSION {
        return Err(format!(
            "unsupported version {}; expected {PERMISSION_CONTRACT_VERSION}",
            contract.version
        ));
    }
    if contract.nodes.is_empty() {
        return Err("node mapping is empty".to_owned());
    }
    for (action_type, definition) in &contract.nodes {
        if action_type.trim().is_empty() {
            return Err("node action type cannot be empty".to_owned());
        }
        if let Some(permission) = &definition.permission
            && permission.name.trim().is_empty()
        {
            return Err(format!("node {action_type:?} has an empty permission name"));
        }
        for rule in &definition.path_rules {
            if rule.config_key.trim().is_empty() {
                return Err(format!("node {action_type:?} has an empty path config key"));
            }
        }
    }
    Ok(contract)
}

fn insert_permission(
    permissions: &mut Vec<PermissionGrant>,
    seen_permissions: &mut BTreeSet<String>,
    permission: PermissionGrant,
) {
    if seen_permissions.insert(permission.name.clone()) {
        permissions.push(permission);
    }
}

fn permission_for_path(access: &PathAccess, path: &str) -> PermissionGrant {
    let unbounded = is_unbounded_path(path);
    let (name, risk) = match (access, unbounded) {
        (PathAccess::Delete, false) => ("file.delete.limited", RiskLevel::High),
        (PathAccess::Delete, true) => ("file.delete.any", RiskLevel::Dangerous),
        (PathAccess::Read, false) => ("file.read", RiskLevel::Medium),
        (PathAccess::Read, true) => ("file.read.any", RiskLevel::Dangerous),
        (PathAccess::Watch, false) => ("file.watch.limited", RiskLevel::Medium),
        (PathAccess::Watch, true) => ("file.watch.any", RiskLevel::Dangerous),
        (PathAccess::Write, false) => ("file.write.limited", RiskLevel::High),
        (PathAccess::Write, true) => ("file.write.any", RiskLevel::Dangerous),
    };
    PermissionGrant {
        name: name.to_owned(),
        risk,
    }
}

/// Reports whether a configured path can escape the script workspace.
///
/// This is the single authority for path risk. The permission calculator uses
/// it to decide between a bounded permission such as `file.write.limited` and
/// the Dangerous `file.write.any`, and the runtime path resolver uses it to
/// decide between workspace confinement and ambient authority. Two independent
/// implementations previously disagreed about several Windows forms, which let
/// a path classify as bounded at approval time and resolve elsewhere at run
/// time.
///
/// Classification is deliberately platform independent. A package authored on
/// Linux can be installed on Windows, so a Windows-specific escape form must
/// classify the same way on both. That makes a few paths that are harmless on
/// Linux, such as a file name containing a colon, classify as unbounded
/// everywhere. The alternative is a risk level that depends on which machine
/// happened to build the package.
#[must_use]
pub fn is_unbounded_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    // A template resolves at run time, so its target cannot be proven bounded.
    if trimmed.contains("{{") && trimmed.contains("}}") {
        return true;
    }

    // Treat both separators as separators regardless of host platform.
    let normalized = trimmed.replace('\\', "/");

    // A leading separator is a filesystem root on either platform. A leading
    // tilde is treated as a home reference even though no supported platform
    // expands it, because a package that relies on expansion should not be
    // presented to the user as a bounded write.
    if normalized.starts_with('/') || normalized == "~" || normalized.starts_with("~/") {
        return true;
    }

    for segment in normalized.split('/').filter(|segment| !segment.is_empty()) {
        if segment == ".." {
            return true;
        }
        // A colon is a drive prefix such as `C:file.txt`, which resolves
        // against that drive's current directory, or an alternate data stream
        // such as `notes.txt:hidden`, which writes to a hidden stream of
        // another file. Neither stays inside the workspace.
        if segment.contains(':') {
            return true;
        }
        if is_reserved_device_name(segment) {
            return true;
        }
    }

    let lowercase = normalized.to_lowercase();
    [
        "/.aws/",
        "/.azure/",
        "/.config/",
        "/.docker/",
        "/.gnupg/",
        "/.kube/",
        "/.ssh/",
        "/etc/",
        "/root/",
        "/var/lib/",
        "/windows/system32/",
        "/windows/syswow64/",
        "/programdata/",
        "/appdata/local/",
        "/appdata/roaming/",
    ]
    .iter()
    .any(|marker| lowercase.contains(marker))
}

/// Reports whether a path segment names a reserved Windows device.
///
/// These names resolve to a device rather than a file in whichever directory
/// they appear, so they never stay inside the workspace. Windows matches them
/// against the segment stem and ignores trailing dots and spaces.
fn is_reserved_device_name(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or_default();
    let stem = stem.trim_end_matches([' ', '.']);
    let stem = stem.to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let bytes = stem.as_bytes();
    bytes.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && bytes[3].is_ascii_digit()
}

fn config_value_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

type ExecutableNode<'a> = (String, &'a serde_json::Map<String, Value>);

fn executable_nodes(program: &Value) -> Result<Vec<ExecutableNode<'_>>, String> {
    let entry = program
        .get("entry")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing entry".to_owned())?;
    let mut nodes = Vec::new();

    if let Some(trigger) = entry.get("trigger") {
        nodes.push(executable_node(trigger)?);
    }
    if let Some(triggers) = entry.get("triggers").and_then(Value::as_array) {
        for trigger in triggers {
            nodes.push(executable_node(trigger)?);
        }
    }
    let steps = entry
        .get("program")
        .and_then(|program| program.get("steps"))
        .and_then(Value::as_array)
        .ok_or_else(|| "missing entry.program.steps".to_owned())?;
    for step in steps {
        nodes.push(executable_node(step)?);
    }
    Ok(nodes)
}

fn executable_node(node: &Value) -> Result<(String, &serde_json::Map<String, Value>), String> {
    let action_type = node
        .get("action_type")
        .and_then(Value::as_str)
        .ok_or_else(|| "node is missing string action_type".to_owned())?;
    let config = node
        .get("config")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("node {action_type} is missing object config"))?;
    Ok((action_type.to_owned(), config))
}

fn enforce_runner_policy(
    permissions: &[PermissionGrant],
    policy: &RunnerPolicy,
) -> Result<(), PermissionValidationError> {
    for permission in permissions {
        if permission.risk == RiskLevel::Dangerous && !policy.allow_dangerous_actions {
            return Err(PermissionValidationError::PolicyBlocked {
                permission: permission.name.clone(),
                reason: "dangerous actions are disabled because security.policy.allow_dangerous_permissions is false".to_owned(),
            });
        }
        if permission.name == "process.shell" && !policy.allow_shell_commands {
            return Err(PermissionValidationError::PolicyBlocked {
                permission: permission.name.clone(),
                reason: "shell commands are disabled because security.policy.allow_shell_commands is false".to_owned(),
            });
        }
    }

    Ok(())
}

pub(crate) fn executable_action_types(program: &Value) -> Result<Vec<String>, String> {
    let entry = program
        .get("entry")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing entry".to_owned())?;
    let mut action_types = Vec::new();

    if let Some(action_type) = entry
        .get("trigger")
        .and_then(|trigger| trigger.get("action_type"))
        .and_then(Value::as_str)
    {
        action_types.push(action_type.to_owned());
    }

    if let Some(triggers) = entry.get("triggers").and_then(Value::as_array) {
        for trigger in triggers {
            if let Some(action_type) = trigger.get("action_type").and_then(Value::as_str) {
                action_types.push(action_type.to_owned());
            }
        }
    }

    let steps = entry
        .get("program")
        .and_then(|program| program.get("steps"))
        .and_then(Value::as_array)
        .ok_or_else(|| "missing entry.program.steps".to_owned())?;

    for step in steps {
        if let Some(action_type) = step.get("action_type").and_then(Value::as_str) {
            action_types.push(action_type.to_owned());
        }
    }

    action_types.sort();
    action_types.dedup();
    Ok(action_types)
}

/// The scope each Variable Operation writes at, read from the declarations.
///
/// This used to read `config.scope` off the node. The node stopped carrying one
/// when a declaration became the only place a variable's scope lives, so every
/// package exported after that change was refused for a missing field it is no
/// longer supposed to have.
///
/// An undeclared name resolves to runtime, the least privileged answer. The run
/// is refused separately for writing a variable nothing declares, so this never
/// has to be the thing that catches it.
pub(crate) fn variable_operation_scopes(
    program: &Value,
    declared_variable_scopes: &BTreeMap<String, VariableScope>,
) -> Result<Vec<VariableScope>, String> {
    let steps = program
        .get("entry")
        .and_then(|entry| entry.get("program"))
        .and_then(|program| program.get("steps"))
        .and_then(Value::as_array)
        .ok_or_else(|| "missing entry.program.steps".to_owned())?;

    Ok(steps
        .iter()
        .filter(|step| {
            step.get("action_type").and_then(Value::as_str) == Some("runtime.set_variable")
        })
        .map(|step| {
            let name = step
                .get("config")
                .and_then(|config| config.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            declared_variable_scopes
                .get(name)
                .copied()
                .unwrap_or(VariableScope::Runtime)
        })
        .collect())
}

pub(crate) fn first_duplicate(values: &[String]) -> Option<String> {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .find(|value| !seen.insert(value.as_str()))
        .cloned()
}

#[cfg(test)]
mod tests;
