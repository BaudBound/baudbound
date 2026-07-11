use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeVariableScope {
    Persistent,
    Global,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionedRuntimeVariable {
    pub value: Value,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSecretDeclaration {
    pub name: String,
    pub required: bool,
    pub value_type: String,
}

pub trait RuntimeStateStore: Send + Sync {
    fn load_variable(
        &self,
        scope: RuntimeVariableScope,
        script_id: &str,
        name: &str,
    ) -> Result<Option<VersionedRuntimeVariable>, String>;

    fn compare_and_set_variable(
        &self,
        scope: RuntimeVariableScope,
        script_id: &str,
        name: &str,
        expected_version: Option<u64>,
        value: &Value,
    ) -> Result<bool, String>;

    fn read_secret(&self, script_id: &str, name: &str) -> Result<Option<Value>, String>;
}
