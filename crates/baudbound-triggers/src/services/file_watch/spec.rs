use std::path::PathBuf;

use serde_json::Value;

use crate::{TriggerError, TriggerRegistration, config_bool};

#[derive(Debug, Clone)]
pub(crate) struct FileWatchSpec {
    pub(super) path: PathBuf,
    pub(super) recursive: bool,
}

impl FileWatchSpec {
    pub(crate) fn from_registration(
        registration: &TriggerRegistration,
    ) -> Result<Self, TriggerError> {
        let path = registration
            .config
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "file watch trigger must define path".to_owned(),
                )
            })?;
        if path.contains("{{") || path.contains("}}") {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "file watch path cannot use runtime variable templates".to_owned(),
            ));
        }

        Ok(Self {
            path: PathBuf::from(path),
            recursive: config_bool(&registration.config, "recursive"),
        })
    }
}
