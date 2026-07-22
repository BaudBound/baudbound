use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use notify::{Event, EventKind, event::ModifyKind};
use serde_json::json;

use crate::{TriggerEvent, TriggerRegistration};

use super::service::FileWatchTarget;

pub(super) fn file_watch_events_from_notify_event(
    registration: &TriggerRegistration,
    target: &FileWatchTarget,
    event: &Event,
) -> Vec<TriggerEvent> {
    let Some(event_name) = file_event_name(&event.kind) else {
        return Vec::new();
    };
    let paths = event
        .paths
        .iter()
        .filter_map(|path| absolute_event_path(path).ok())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Vec::new();
    }

    if event_name == "renamed" {
        if !paths.iter().any(|path| target.accepts(path)) {
            return Vec::new();
        }
        let output_path = paths
            .last()
            .expect("non-empty rename paths must have a final path");
        return vec![file_watch_event(
            registration,
            target.watched_path(),
            output_path,
            event_name,
        )];
    }

    paths
        .into_iter()
        .filter(|path| target.accepts(path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|path| file_watch_event(registration, target.watched_path(), &path, event_name))
        .collect()
}

pub(crate) fn file_watch_event(
    registration: &TriggerRegistration,
    watched_path: &Path,
    event_path: &Path,
    event_name: &str,
) -> TriggerEvent {
    TriggerEvent {
        action_type: registration.action_type.clone(),
        node_id: registration.node_id.clone(),
        payload: json!({
            "event": event_name,
            "path": event_path.to_string_lossy(),
            "watched_path": watched_path.to_string_lossy(),
        }),
        script_id: registration.script_id.clone(),
    }
}

fn file_event_name(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Access(_) => None,
        EventKind::Create(_) => Some("created"),
        EventKind::Modify(ModifyKind::Name(_)) => Some("renamed"),
        EventKind::Modify(_) => Some("modified"),
        EventKind::Remove(_) => Some("deleted"),
        EventKind::Any | EventKind::Other => Some("modified"),
    }
}

fn absolute_event_path(path: &Path) -> std::io::Result<PathBuf> {
    std::path::absolute(path)
}
