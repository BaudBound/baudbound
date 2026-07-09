use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{InstalledScript, ScriptApproval};

const STORAGE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct StorageIndex {
    #[serde(default = "storage_format_version")]
    pub(crate) format_version: u32,
    #[serde(default)]
    pub(crate) scripts: BTreeMap<String, InstalledScript>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ApprovalIndex {
    #[serde(default = "storage_format_version")]
    pub(crate) format_version: u32,
    #[serde(default)]
    pub(crate) approvals: BTreeMap<String, ScriptApproval>,
}

fn storage_format_version() -> u32 {
    STORAGE_FORMAT_VERSION
}
