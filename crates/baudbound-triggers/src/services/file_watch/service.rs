use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::SyncSender,
    time::Duration,
};

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics,
    try_send_trigger_event,
};

use super::{event::file_watch_events_from_notify_event, spec::FileWatchSpec};

pub struct FileWatchService {
    watchers: Vec<Debouncer<RecommendedWatcher, RecommendedCache>>,
}

const FILE_WATCH_DEBOUNCE: Duration = Duration::from_millis(50);

impl FileWatchService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            watchers: Vec::new(),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: SyncSender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let mut watchers = Vec::new();

        for registration in registrations {
            if registration.action_type != "trigger.file_watch" {
                continue;
            }

            let spec = FileWatchSpec::from_registration(&registration)?;
            let target = FileWatchTarget::from_spec(&registration, &spec)?;
            let callback_registration = registration.clone();
            let callback_target = target.clone();
            let callback_sender = sender.clone();
            let mut watcher = new_debouncer(
                FILE_WATCH_DEBOUNCE,
                None,
                move |result: DebounceEventResult| match result {
                    Ok(events) => {
                        for debounced_event in events {
                            for event in file_watch_events_from_notify_event(
                                &callback_registration,
                                &callback_target,
                                &debounced_event.event,
                            ) {
                                try_send_trigger_event(&callback_sender, event, "file watch");
                            }
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            tracing::warn!(
                                "file watch trigger {} failed to read event: {}",
                                callback_registration.node_id,
                                error
                            );
                        }
                    }
                },
            )
            .map_err(|source| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    format!("failed to create file watcher: {source}"),
                )
            })?;
            watcher
                .watch(target.watch_root(), target.recursive_mode())
                .map_err(|source| {
                    TriggerError::Failed(
                        registration.node_id.clone(),
                        format!(
                            "failed to watch {}: {source}",
                            target.watched_path().display()
                        ),
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
pub(super) struct FileWatchTarget {
    kind: FileWatchTargetKind,
    recursive: bool,
    watch_root: PathBuf,
    watched_path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
enum FileWatchTargetKind {
    Directory,
    File,
}

impl FileWatchTarget {
    pub(super) fn from_spec(
        registration: &TriggerRegistration,
        spec: &FileWatchSpec,
    ) -> Result<Self, TriggerError> {
        let watched_path = std::path::absolute(&spec.path).map_err(|source| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!(
                    "failed to resolve file watch path {}: {source}",
                    spec.path.display()
                ),
            )
        })?;
        let metadata = fs::metadata(&watched_path).map_err(|source| {
            TriggerError::Failed(
                registration.node_id.clone(),
                format!(
                    "file watch path {} is not accessible: {source}",
                    watched_path.display()
                ),
            )
        })?;
        if metadata.is_file() {
            let watch_root = watched_path.parent().ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    format!(
                        "file watch path {} has no parent directory",
                        watched_path.display()
                    ),
                )
            })?;
            return Ok(Self {
                kind: FileWatchTargetKind::File,
                recursive: false,
                watch_root: watch_root.to_path_buf(),
                watched_path,
            });
        }
        if metadata.is_dir() {
            return Ok(Self {
                kind: FileWatchTargetKind::Directory,
                recursive: spec.recursive,
                watch_root: watched_path.clone(),
                watched_path,
            });
        }
        Err(TriggerError::Failed(
            registration.node_id.clone(),
            format!(
                "file watch path {} must be a regular file or directory",
                watched_path.display()
            ),
        ))
    }

    pub(super) fn accepts(&self, path: &Path) -> bool {
        match self.kind {
            FileWatchTargetKind::File => path == self.watched_path,
            FileWatchTargetKind::Directory if self.recursive => {
                path != self.watched_path && path.starts_with(&self.watched_path)
            }
            FileWatchTargetKind::Directory => path.parent() == Some(self.watched_path.as_path()),
        }
    }

    pub(super) fn recursive_mode(&self) -> RecursiveMode {
        if self.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        }
    }

    pub(super) fn watch_root(&self) -> &Path {
        &self.watch_root
    }

    pub(super) fn watched_path(&self) -> &Path {
        &self.watched_path
    }
}
