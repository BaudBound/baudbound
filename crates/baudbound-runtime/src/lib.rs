//! Runtime primitives for executing BaudBound script graphs.

pub use baudbound_script::is_user_identifier;

mod execution;
mod resource_limit;
mod runtime;
mod safe_regex;
mod value_type;

pub use execution::*;
pub use resource_limit::ResourceLimit;
pub use runtime::{
    RuntimeCancellationSubscription, RuntimeCancellationToken, RuntimeDefaultVariable,
    RuntimeDefaultVariableScope, RuntimeScriptSettings, RuntimeSecretDeclaration,
    RuntimeStateStore, RuntimeVariableScope, VersionedRuntimeVariable, resolve_template_value,
};
pub use safe_regex::{compile_cached_regex, compile_safe_regex, max_simulation_regex_input_bytes};
pub use value_type::{ValueType, validate_value, value_type_name};

#[cfg(test)]
mod tests;
