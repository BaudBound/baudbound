use std::collections::BTreeMap;

use serde_json::Value;

use crate::{TriggerError, TriggerRegistration};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProcessStartedSpec {
    pub(crate) id: RegistrationId,
    pub(crate) match_mode: ProcessMatchMode,
    pub(crate) registration: TriggerRegistration,
    pub(crate) target: String,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RegistrationId {
    pub(super) node_id: String,
    pub(super) script_id: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ProcessMatchMode {
    ExecutablePath,
    ProcessName,
    WindowTitle,
}

impl ProcessStartedSpec {
    pub(crate) fn from_registration(
        registration: TriggerRegistration,
    ) -> Result<Self, TriggerError> {
        let target = registration
            .config
            .get("target")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "process started trigger must define target".to_owned(),
                )
            })?
            .to_owned();
        let match_mode = match registration
            .config
            .get("matchMode")
            .and_then(Value::as_str)
            .unwrap_or("process_name")
            .trim()
        {
            "executable_path" => ProcessMatchMode::ExecutablePath,
            "process_name" => ProcessMatchMode::ProcessName,
            "window_title" if cfg!(windows) => ProcessMatchMode::WindowTitle,
            "window_title" => {
                return Err(TriggerError::Failed(
                    registration.node_id.clone(),
                    "window title process start matching requires Windows Desktop".to_owned(),
                ));
            }
            unsupported => {
                return Err(TriggerError::Failed(
                    registration.node_id.clone(),
                    format!("unsupported process started match mode {unsupported:?}"),
                ));
            }
        };

        Ok(Self {
            id: RegistrationId {
                node_id: registration.node_id.clone(),
                script_id: registration.script_id.clone(),
            },
            match_mode,
            registration,
            target,
        })
    }
}

pub(super) fn collect_specs(
    registrations: impl IntoIterator<Item = TriggerRegistration>,
) -> Result<BTreeMap<RegistrationId, ProcessStartedSpec>, TriggerError> {
    let mut specs = BTreeMap::new();
    for registration in registrations
        .into_iter()
        .filter(|registration| registration.action_type == "trigger.process_started")
    {
        let spec = ProcessStartedSpec::from_registration(registration)?;
        if specs.insert(spec.id.clone(), spec.clone()).is_some() {
            return Err(TriggerError::Failed(
                spec.registration.node_id,
                "duplicate process started trigger registration".to_owned(),
            ));
        }
    }
    Ok(specs)
}
