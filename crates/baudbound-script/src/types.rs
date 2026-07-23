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
    pub variables: Vec<DefaultVariable>,
    #[serde(default)]
    pub secrets: Vec<SecretDeclaration>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DefaultVariable {
    pub name: String,
    pub scope: String,
    #[serde(rename = "type")]
    pub value_type: String,
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
    pub target_runtime: String,
}

pub type Program = Value;
pub type EditorMetadata = Value;
