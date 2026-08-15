use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub format_version: u32,
    pub script_language_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub source: String,
    pub created_with: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub minimum_runner_version: String,
    pub version: String,
    #[serde(default)]
    pub repository_url: String,
    #[serde(default)]
    pub assets: Vec<ManifestAsset>,
    #[serde(default)]
    pub variables: Vec<DeclaredVariable>,
    #[serde(default)]
    pub settings: Vec<ScriptSettingDeclaration>,
    #[serde(default)]
    pub secrets: Vec<SecretDeclaration>,
}

/// Where a declared variable lives, and for how long.
///
/// This is an enum rather than the string the manifest carries so that the
/// three places deriving something from a scope — the permission it requires,
/// the runtime store it reads, the install conflict it can cause — are
/// exhaustive matches the compiler checks. As strings they were four separate
/// opinions about what an unrecognised scope meant, and they disagreed: the
/// permission calculator refused one while the runtime silently read it as
/// `Runtime`. They agreed in practice only because the manifest schema had
/// already refused anything else.
///
/// `secret` is deliberately absent. A secret is not a variable scope; it is a
/// separate manifest declaration that grants `secret.read` and nothing that
/// writes.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum VariableScope {
    Runtime,
    Persistent,
    Global,
}

impl VariableScope {
    /// The manifest spelling, for an error message or a stored row.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Persistent => "persistent",
            Self::Global => "global",
        }
    }
}

impl std::fmt::Display for VariableScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DeclaredVariable {
    pub name: String,
    pub scope: VariableScope,
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    #[serde(default)]
    pub description: String,
    pub value: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SecretDeclaration {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ScriptSettingDeclaration {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestAsset {
    pub id: String,
    pub kind: String,
    pub media_type: String,
    pub name: String,
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Permissions {
    #[serde(default)]
    pub declared_permissions: Vec<String>,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Dangerous,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Capabilities {
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub target_runtimes: Vec<String>,
}

pub type Program = Value;
pub type EditorMetadata = Value;
