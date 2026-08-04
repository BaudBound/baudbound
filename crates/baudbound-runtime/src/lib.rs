//! Runtime primitives for executing BaudBound script graphs.

pub use baudbound_script::is_user_identifier;

mod execution;
mod resource_limit;
mod runtime;
mod safe_regex;

pub use execution::*;
pub use resource_limit::ResourceLimit;
pub use runtime::{
    RuntimeCancellationSubscription, RuntimeCancellationToken, RuntimeDefaultVariable,
    RuntimeDefaultVariableScope, RuntimeScriptSettings, RuntimeSecretDeclaration,
    RuntimeStateStore, RuntimeVariableScope, VersionedRuntimeVariable, resolve_template_value,
};
pub use safe_regex::{compile_safe_regex, max_simulation_regex_input_bytes};

#[cfg(test)]
mod tests;
