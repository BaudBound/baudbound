//! Proves every cast in a config succeeds before the config is resolved.
//!
//! Template resolution is used in roughly 35 places and is infallible.
//! Threading a Result through all of them would obscure the change, so casts
//! are validated once per node beforehand. This also means a cast failure
//! happens before the node does anything, so no HTTP request is sent and no
//! file is partially written.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::runtime::{resolve_reference, split_cast};
use crate::{RuntimeError, ValueType, cast_value};

/// Matches the template pattern used by the resolver: the first `{{`, then
/// the next `}}`, repeated until no further opening delimiter is found. This
/// must stay in lockstep with `runtime::templates::render_template`, or a
/// cast could slip past this pre-pass and fail silently during resolution
/// instead of stopping the run here.
fn for_each_template(
    text: &str,
    mut visit: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let mut remaining = text;
    while let Some(start) = remaining.find("{{") {
        let after = &remaining[start + 2..];
        let Some(end) = after.find("}}") else {
            break;
        };
        visit(&after[..end])?;
        remaining = &after[end + 2..];
    }
    Ok(())
}

/// Validates one `{{reference}}` or `{{reference|target}}` expression.
///
/// An expression without a cast target has nothing to prove; the resolver
/// leaves a plain reference as-is. `resolve_reference` is the same
/// accessor-path lookup the resolver uses to find the value a dotted or
/// bracketed reference points at, so this checks the exact value that would
/// otherwise be cast during resolution.
fn validate_expression(
    expression: &str,
    variables: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let (reference, Some(target)) = split_cast(expression) else {
        return Ok(());
    };
    let target = target
        .parse::<ValueType>()
        .map_err(|_| format!("unknown cast target in {{{{{expression}}}}}"))?;
    match resolve_reference(reference, variables) {
        Some(resolved) => cast_value(resolved, target)
            .map(|_| ())
            .map_err(|reason| format!("variable \"{reference}\" {reason}")),
        None => Err(format!("variable \"{reference}\" is not set")),
    }
}

/// Proves every cast inside an already parsed value succeeds.
///
/// Two call sites parse a config string as JSON and then resolve templates in
/// the parsed result. An escaped brace is invisible to the scan over the raw
/// string but becomes a real template once parsed, so the parsed value has to
/// be proven again.
pub(crate) fn validate_value_casts(
    node_id: &str,
    value: &Value,
    variables: &BTreeMap<String, Value>,
) -> Result<(), RuntimeError> {
    walk(value, variables).map_err(|message| RuntimeError::Cast {
        node_id: node_id.to_owned(),
        message,
    })
}

pub(crate) fn validate_config_casts(
    node_id: &str,
    config: &Map<String, Value>,
    variables: &BTreeMap<String, Value>,
) -> Result<(), RuntimeError> {
    for value in config.values() {
        walk(value, variables).map_err(|message| RuntimeError::Cast {
            node_id: node_id.to_owned(),
            message,
        })?;
    }
    Ok(())
}

fn walk(value: &Value, variables: &BTreeMap<String, Value>) -> Result<(), String> {
    match value {
        Value::String(text) => for_each_template(text, |expression| {
            validate_expression(expression, variables)
        }),
        Value::Array(items) => items.iter().try_for_each(|item| walk(item, variables)),
        Value::Object(fields) => fields.iter().try_for_each(|(key, field)| {
            // The resolver renders a dynamic object key through the same
            // template machinery as a value, so a cast in the key must
            // be proven here too.
            for_each_template(key, |expression| validate_expression(expression, variables))?;
            walk(field, variables)
        }),
        _ => Ok(()),
    }
}

/// Proves every cast inside one template string succeeds.
///
/// Trigger configuration is resolved before a run exists, so it cannot use the
/// per-node pre-pass. Without this a failing cast there would resolve to the
/// literal template text and be registered as a schedule interval, a hotkey or
/// a webhook response body.
pub fn validate_template_casts(
    template: &str,
    variables: &BTreeMap<String, Value>,
) -> Result<(), String> {
    for_each_template(template, |expression| {
        validate_expression(expression, variables)
    })
}
