use std::{
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Value, json};

use crate::{TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics};

pub struct FileWatchService {
    watchers: Vec<RecommendedWatcher>,
}

impl FileWatchService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            watchers: Vec::new(),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: Sender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let mut watchers = Vec::new();

        for registration in registrations {
            if registration.action_type != "trigger.file_watch" {
                continue;
            }

            let spec = FileWatchSpec::from_registration(&registration)?;
            let callback_registration = registration.clone();
            let callback_watched_path = spec.path.clone();
            let callback_sender = sender.clone();
            let mut watcher = RecommendedWatcher::new(
                move |result: notify::Result<Event>| match result {
                    Ok(event) => {
                        for event in file_watch_events_from_notify_event(
                            &callback_registration,
                            &callback_watched_path,
                            &event,
                        ) {
                            let _ = callback_sender.send(event);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            "file watch trigger {} failed to read event: {}",
                            callback_registration.node_id,
                            error
                        );
                    }
                },
                Config::default(),
            )
            .map_err(|source| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    format!("failed to create file watcher: {source}"),
                )
            })?;
            watcher
                .watch(&spec.path, RecursiveMode::NonRecursive)
                .map_err(|source| {
                    TriggerError::Failed(
                        registration.node_id.clone(),
                        format!("failed to watch {}: {source}", spec.path.display()),
                    )
                })?;
            watchers.push(watcher);
        }

        Ok(Self { watchers })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.watchers.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.watchers.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "file watcher")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FileWatchSpec {
    path: PathBuf,
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
        })
    }
}

fn file_watch_events_from_notify_event(
    registration: &TriggerRegistration,
    watched_path: &Path,
    event: &Event,
) -> Vec<TriggerEvent> {
    let event_name = file_event_name(&event.kind);
    let paths = if event.paths.is_empty() {
        vec![watched_path.to_path_buf()]
    } else {
        event.paths.clone()
    };

    paths
        .into_iter()
        .map(|path| file_watch_event(registration, watched_path, &path, event_name))
        .collect()
}

pub(crate) fn file_watch_event(
    registration: &TriggerRegistration,
    watched_path: &Path,
    event_path: &Path,
    event_name: &str,
) -> TriggerEvent {
    TriggerEvent {
        node_id: registration.node_id.clone(),
        payload: json!({
            "event": event_name,
            "path": event_path.to_string_lossy(),
            "watched_path": watched_path.to_string_lossy(),
        }),
        script_id: registration.script_id.clone(),
    }
}

fn file_event_name(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Access(_) => "accessed",
        EventKind::Create(_) => "created",
        EventKind::Modify(_) => "modified",
        EventKind::Remove(_) => "removed",
        EventKind::Any | EventKind::Other => "changed",
    }
}
