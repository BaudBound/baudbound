use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

use baudbound_script::ScriptPackage;
use baudbound_storage::{InstalledScript, ScriptStore};
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
