//! Runtime primitives for executing BaudBound script graphs.

mod execution;
mod runtime;

pub use execution::*;
pub use runtime::{
    RuntimeCancellationToken, RuntimeDefaultVariable, RuntimeDefaultVariableScope,
    RuntimeScriptSettings, RuntimeSecretDeclaration, RuntimeStateStore, RuntimeVariableScope,
    VersionedRuntimeVariable, resolve_template_value,
};

#[cfg(test)]
mod tests;
