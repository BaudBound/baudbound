//! Runtime primitives for executing BaudBound script graphs.

pub use baudbound_script::is_user_identifier;

mod cast;
mod execution;
mod resource_limit;
mod runtime;
mod safe_regex;
mod value_type;

pub use cast::cast_value;
pub use execution::*;
pub use resource_limit::ResourceLimit;
pub use runtime::{
    RuntimeCancellationSubscription, RuntimeCancellationToken, RuntimeDeclaredScope,
    RuntimeDeclaredVariable, RuntimeScriptSettings, RuntimeSecretDeclaration, RuntimeStateStore,
    RuntimeVariableScope, VersionedRuntimeVariable, format_datetime, resolve_template_value,
    validate_datetime_pattern,
};
pub use safe_regex::{compile_cached_regex, compile_safe_regex, max_simulation_regex_input_bytes};
pub use value_type::{ValueType, validate_value, value_type_name};

#[cfg(test)]
mod tests;
