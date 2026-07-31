use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use baudbound_runtime::resolve_template_value;
use baudbound_script::ScriptPackage;
use baudbound_storage::{InstalledScript, ScriptStore, StoredVariableScope};
use baudbound_triggers::{TriggerDispatcher, TriggerEvent, TriggerRegistration};
use serde_json::Value;

use crate::{CoreError, RunReport, RunnerCore};

pub struct CoreTriggerDispatcher<'core, S: ScriptStore> {
    pub(crate) core: &'core RunnerCore,
    pub(crate) store: &'core S,
}

impl<S: ScriptStore> TriggerDispatcher for CoreTriggerDispatcher<'_, S> {
    fn dispatch(&self, event: TriggerEvent) -> Result<RunReport, baudbound_triggers::TriggerError> {
        let script_id = event.script_id.clone();
        let node_id = event.node_id.clone();
        self.core
            .dispatch_trigger_event(self.store, event)
            .map_err(|source| {
                baudbound_triggers::TriggerError::Failed(
                    format!("{script_id}:{node_id}"),
                    source.to_string(),
                )
            })
    }
}

pub(crate) fn trigger_registrations_from_package(
    store: &impl ScriptStore,
    installed: &InstalledScript,
    package: &ScriptPackage,
) -> Result<Vec<TriggerRegistration>, CoreError> {
    let entry = package
        .program
        .get("entry")
        .and_then(Value::as_object)
        .ok_or_else(|| CoreError::InvalidTriggerRegistration("missing entry".to_owned()))?;

    let mut trigger_values = Vec::new();
    if let Some(trigger) = entry.get("trigger") {
        trigger_values.push(trigger);
    }
    if let Some(triggers) = entry.get("triggers").and_then(Value::as_array) {
        trigger_values.extend(triggers);
    }

    let mut seen_node_ids = BTreeSet::new();
    let mut registrations = Vec::new();
    let pre_trigger_values = trigger_values
        .iter()
        .any(|trigger| {
            trigger
                .get("action_type")
                .and_then(Value::as_str)
                .is_some_and(|action_type| !pre_trigger_template_fields(action_type).is_empty())
        })
        .then(|| pre_trigger_variables(store, installed, package))
        .transpose()?;
    for trigger in trigger_values {
        let action_type = trigger
            .get("action_type")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CoreError::InvalidTriggerRegistration("trigger is missing action_type".to_owned())
            })?;
        if !action_type.starts_with("trigger.") {
            return Err(CoreError::InvalidTriggerRegistration(format!(
                "{action_type} is not a trigger action_type"
            )));
        }

        let node_id = trigger.get("id").and_then(Value::as_str).ok_or_else(|| {
            CoreError::InvalidTriggerRegistration("trigger is missing id".to_owned())
        })?;
        if !seen_node_ids.insert(node_id.to_owned()) {
            continue;
        }

        let runner_type = trigger
            .get("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| action_type.trim_start_matches("trigger.").to_owned());
        let mut config = trigger
            .get("config")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        if let Some(variables) = pre_trigger_values.as_ref() {
            resolve_pre_trigger_config(action_type, &mut config, variables);
        }
        if action_type == "trigger.file_watch" {
            resolve_limited_file_watch_path(installed, &mut config)?;
        }

        registrations.push(TriggerRegistration {
            action_type: action_type.to_owned(),
            config,
            node_id: node_id.to_owned(),
            runner_type,
            script_id: installed.id.clone(),
            script_name: installed.name.clone(),
        });
    }

    Ok(registrations)
}

fn pre_trigger_variables(
    store: &impl ScriptStore,
    installed: &InstalledScript,
    package: &ScriptPackage,
) -> Result<BTreeMap<String, Value>, CoreError> {
    let mut variables = BTreeMap::new();
    for variable in &package.manifest.variables {
        let value = if variable.scope == "persistent" {
            store
                .load_variable(
                    StoredVariableScope::Persistent,
                    &installed.id,
                    &variable.name,
                )?
                .map(|stored| stored.value)
                .unwrap_or_else(|| variable.value.clone())
        } else {
            variable.value.clone()
        };
        variables.insert(variable.name.clone(), value);
    }

    let configured_settings = store
        .list_script_settings(&installed.id)?
        .into_iter()
        .map(|setting| (setting.name, setting.value))
        .collect::<BTreeMap<_, _>>();
    let settings = package
        .manifest
        .settings
        .iter()
        .map(|declaration| {
            (
                declaration.name.clone(),
                configured_settings
                    .get(&declaration.name)
                    .cloned()
                    .or_else(|| declaration.default_value.clone())
                    .unwrap_or(Value::Null),
            )
        })
        .collect();
    variables.insert("settings".to_owned(), Value::Object(settings));
    Ok(variables)
}

fn resolve_pre_trigger_config(
    action_type: &str,
    config: &mut Value,
    variables: &BTreeMap<String, Value>,
) {
    for field in pre_trigger_template_fields(action_type) {
        let Some(template) = config.get(*field).and_then(Value::as_str) else {
            continue;
        };
        let resolved = resolve_template_value(template, variables);
        if let Some(value) = config.get_mut(*field) {
            *value = resolved;
        }
    }
}

fn pre_trigger_template_fields(action_type: &str) -> &'static [&'static str] {
    match action_type {
        "trigger.schedule" => &["every"],
        "trigger.file_watch" => &["path"],
        "trigger.process_started" => &["target"],
        "trigger.hotkey" => &["key"],
        "trigger.webhook" => &["timeoutResponseContentType", "timeoutResponseBody"],
        _ => &[],
    }
}

fn resolve_limited_file_watch_path(
    installed: &InstalledScript,
    config: &mut Value,
) -> Result<(), CoreError> {
    let Some(path) = config
        .get("path")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return Ok(());
    };
    let configured = Path::new(&path);
    if configured.is_absolute()
        || configured
            .components()
            .any(|component| component == Component::ParentDir)
        || path
            .replace('\\', "/")
            .split('/')
            .any(|component| component == "..")
    {
        return Ok(());
    }
    let scripts_directory = installed.package_path.parent().ok_or_else(|| {
        CoreError::InvalidTriggerRegistration(
            "installed package has no scripts directory".to_owned(),
        )
    })?;
    let runner_home = scripts_directory.parent().ok_or_else(|| {
        CoreError::InvalidTriggerRegistration(
            "installed package has no runner directory".to_owned(),
        )
    })?;
    let workspace = runner_home.join("workspaces").join(&installed.id);
    fs::create_dir_all(&workspace).map_err(|source| {
        CoreError::InvalidTriggerRegistration(format!(
            "failed to create script workspace {}: {source}",
            workspace.display()
        ))
    })?;
    let canonical_workspace = workspace.canonicalize().map_err(|source| {
        CoreError::InvalidTriggerRegistration(format!(
            "failed to resolve script workspace {}: {source}",
            workspace.display()
        ))
    })?;
    let resolved = canonical_workspace.join(configured);
    let existing_ancestor = nearest_existing_ancestor(&resolved);
    let canonical_ancestor = existing_ancestor.canonicalize().map_err(|source| {
        CoreError::InvalidTriggerRegistration(format!(
            "failed to resolve file watch path {}: {source}",
            resolved.display()
        ))
    })?;
    if !canonical_ancestor.starts_with(&canonical_workspace) {
        return Err(CoreError::InvalidTriggerRegistration(format!(
            "limited file watch path {path} escapes the script workspace"
        )));
    }
    config["path"] = Value::String(resolved.to_string_lossy().into_owned());
    Ok(())
}

fn nearest_existing_ancestor(path: &Path) -> PathBuf {
    let mut current = path;
    while !current.exists() {
        let Some(parent) = current.parent() else {
            return path.to_path_buf();
        };
        current = parent;
    }
    current.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_schedule_values_available_before_trigger_dispatch() {
        let variables = BTreeMap::from([
            ("interval".to_owned(), json!(2.5)),
            ("settings".to_owned(), json!({ "interval": 4 })),
        ]);

        let mut default_config = json!({ "every": "{{interval}}", "unit": "seconds" });
        resolve_pre_trigger_config("trigger.schedule", &mut default_config, &variables);
        assert_eq!(default_config["every"], json!(2.5));

        let mut setting_config = json!({ "every": "{{settings.interval}}", "unit": "seconds" });
        resolve_pre_trigger_config("trigger.schedule", &mut setting_config, &variables);
        assert_eq!(setting_config["every"], json!(4));
    }

    #[test]
    fn leaves_unavailable_schedule_values_unresolved_for_service_validation() {
        let mut config = json!({ "every": "{{node.output}}", "unit": "seconds" });

        resolve_pre_trigger_config("trigger.schedule", &mut config, &BTreeMap::new());

        assert_eq!(config["every"], json!("{{node.output}}"));
    }

    #[test]
    fn resolves_only_declared_pre_trigger_fields() {
        let variables = BTreeMap::from([
            ("path".to_owned(), json!("inbox")),
            ("key".to_owned(), json!("Ctrl+F8")),
            ("name".to_owned(), json!("must-not-resolve")),
        ]);

        let mut file_watch = json!({ "path": "{{path}}", "recursive": false });
        resolve_pre_trigger_config("trigger.file_watch", &mut file_watch, &variables);
        assert_eq!(file_watch["path"], json!("inbox"));

        let mut hotkey = json!({ "key": "{{key}}" });
        resolve_pre_trigger_config("trigger.hotkey", &mut hotkey, &variables);
        assert_eq!(hotkey["key"], json!("Ctrl+F8"));

        for (action_type, field) in [
            ("trigger.serial_input", "deviceId"),
            ("trigger.webhook", "hookName"),
            ("trigger.websocket", "path"),
        ] {
            let mut config = json!({ (field): "{{name}}" });
            resolve_pre_trigger_config(action_type, &mut config, &variables);
            assert_eq!(config[field], json!("{{name}}"));
        }
    }
}
