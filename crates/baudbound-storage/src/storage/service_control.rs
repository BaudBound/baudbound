use std::{fs, io};

use serde::{Deserialize, Serialize};

use crate::{
    FilesystemScriptStore, StorageError,
    storage::filesystem::{current_unix_timestamp, write_atomic},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ServiceControlCommand {
    Reload,
    Stop,
}

impl ServiceControlCommand {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reload => "reload",
            Self::Stop => "stop",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "reload" => Some(Self::Reload),
            "stop" => Some(Self::Stop),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConsumedServiceControl {
    Command(ServiceControlCommand),
    Ignored { reason: String },
}

#[derive(Debug, Deserialize, Serialize)]
struct ServiceControlRequest {
    command: String,
    requested_at_unix: u64,
    requested_by_pid: u32,
    target_pid: u32,
}

impl FilesystemScriptStore {
    pub fn write_service_status(&self, status: &serde_json::Value) -> Result<(), StorageError> {
        self.ensure_layout()?;
        let path = self.service_status_path();
        let content = serde_json::to_vec_pretty(status).map_err(|source| StorageError::Json {
            path: path.clone(),
            source,
        })?;
        write_atomic(&path, &content)
    }

    pub fn read_service_status(&self) -> Result<Option<serde_json::Value>, StorageError> {
        let path = self.service_status_path();
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content)
                .map(Some)
                .map_err(|source| StorageError::Json { path, source }),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(StorageError::Io { path, source }),
        }
    }

    pub fn clear_service_status(&self) -> Result<bool, StorageError> {
        let path = self.service_status_path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(StorageError::Io { path, source }),
        }
    }

    pub fn write_service_control_request(
        &self,
        request: &serde_json::Value,
    ) -> Result<(), StorageError> {
        self.ensure_layout()?;
        let path = self.service_control_path();
        let content = serde_json::to_vec_pretty(request).map_err(|source| StorageError::Json {
            path: path.clone(),
            source,
        })?;
        write_atomic(&path, &content)
    }

    pub fn request_service_control(
        &self,
        command: ServiceControlCommand,
        requested_by_pid: u32,
    ) -> Result<(), StorageError> {
        let request = ServiceControlRequest {
            command: command.as_str().to_owned(),
            requested_at_unix: current_unix_timestamp(),
            requested_by_pid,
            target_pid: self.running_service_pid()?,
        };
        let request = serde_json::to_value(request).map_err(|source| StorageError::Json {
            path: self.service_control_path(),
            source,
        })?;
        self.write_service_control_request(&request)
    }

    pub fn running_service_pid(&self) -> Result<u32, StorageError> {
        let service_status = self.read_service_status()?.ok_or_else(|| {
            StorageError::Operation("no runner service status has been written yet".to_owned())
        })?;
        let state = service_status
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        if state != "running" {
            return Err(StorageError::Operation(format!(
                "runner service is not running; latest state is {state:?}"
            )));
        }
        let pid = service_status
            .get("pid")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                StorageError::Operation("runner service status does not contain a pid".to_owned())
            })?;
        u32::try_from(pid)
            .map_err(|_| StorageError::Operation("runner service pid is out of range".to_owned()))
    }

    pub fn consume_service_control_request(
        &self,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        let path = self.service_control_path();
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(StorageError::Io { path, source }),
        };

        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(StorageError::Io {
                    path: path.clone(),
                    source,
                });
            }
        }

        serde_json::from_str(&content)
            .map(Some)
            .map_err(|source| StorageError::Json { path, source })
    }

    pub fn consume_service_control_request_for_pid(
        &self,
        current_pid: u32,
    ) -> Result<Option<ConsumedServiceControl>, StorageError> {
        let Some(request) = self.consume_service_control_request()? else {
            return Ok(None);
        };
        let request =
            serde_json::from_value::<ServiceControlRequest>(request).map_err(|source| {
                StorageError::Json {
                    path: self.service_control_path(),
                    source,
                }
            })?;

        if request.target_pid != current_pid {
            return Ok(Some(ConsumedServiceControl::Ignored {
                reason: format!(
                    "request targets pid {}, but this process is {current_pid}",
                    request.target_pid
                ),
            }));
        }

        let Some(command) = ServiceControlCommand::from_str(&request.command) else {
            return Ok(Some(ConsumedServiceControl::Ignored {
                reason: format!("unknown command {:?}", request.command),
            }));
        };

        Ok(Some(ConsumedServiceControl::Command(command)))
    }
}
