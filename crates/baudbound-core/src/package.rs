use std::path::Path;

use baudbound_script::{PackageSummary, RiskLevel, ScriptPackage};
use baudbound_security::{
    RunnerPolicy, RuntimeDeclarationRequirements, SecurityValidationError,
    validate_program_capabilities_with_declarations,
    validate_program_permissions_with_declarations,
};
use baudbound_storage::ImportScriptRequest;

#[derive(Debug, Clone)]
pub struct PackageInspection {
    pub entries: Vec<String>,
    pub summary: PackageSummary,
}

impl PackageInspection {
    pub(crate) fn from_package(package: ScriptPackage) -> Self {
        Self {
            entries: package
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect(),
            summary: package.summary(),
        }
    }
}

pub(crate) fn import_request_from_package(
    path: &Path,
    package: ScriptPackage,
) -> ImportScriptRequest {
    let summary = package.summary();
    ImportScriptRequest {
        id: package.manifest.id,
        name: summary.script_name,
        package_source: path.to_path_buf(),
        package_format_version: summary.package_format_version,
        script_language_version: summary.script_language_version,
        target_runtime: summary.target_runtime,
        asset_count: summary.asset_count,
        risk_level: risk_level_name(&package.permissions.risk_level).to_owned(),
    }
}

pub(crate) fn validate_package_security(
    package: &ScriptPackage,
    policy: &RunnerPolicy,
) -> Result<(), SecurityValidationError> {
    let requirements = RuntimeDeclarationRequirements {
        has_persistent_default_variables: package
            .manifest
            .variables
            .iter()
            .any(|variable| variable.scope == "persistent"),
        has_runtime_default_variables: package
            .manifest
            .variables
            .iter()
            .any(|variable| variable.scope == "runtime"),
        has_secret_declarations: !package.manifest.secrets.is_empty(),
    };
    validate_program_permissions_with_declarations(
        &package.program,
        &package.permissions.declared_permissions,
        security_risk_level(&package.permissions.risk_level),
        policy,
        requirements,
    )?;
    validate_program_capabilities_with_declarations(
        &package.program,
        &package.capabilities.required_capabilities,
        requirements,
    )?;
    Ok(())
}

fn risk_level_name(risk_level: &RiskLevel) -> &'static str {
    match risk_level {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Dangerous => "dangerous",
    }
}

fn security_risk_level(risk_level: &RiskLevel) -> baudbound_security::RiskLevel {
    match risk_level {
        RiskLevel::Low => baudbound_security::RiskLevel::Low,
        RiskLevel::Medium => baudbound_security::RiskLevel::Medium,
        RiskLevel::High => baudbound_security::RiskLevel::High,
        RiskLevel::Dangerous => baudbound_security::RiskLevel::Dangerous,
    }
}
