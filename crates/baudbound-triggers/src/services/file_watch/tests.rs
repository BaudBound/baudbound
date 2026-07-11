use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};

use notify::{
    Event, EventKind,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};
use serde_json::json;

use super::{
    FileWatchService, event::file_watch_events_from_notify_event, service::FileWatchTarget,
    spec::FileWatchSpec,
};
use crate::{TriggerEvent, TriggerRegistration};

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn normalizes_create_modify_delete_and_rename_events() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should exist");
    let watched_path = temporary_directory.path().join("watched.txt");
    fs::write(&watched_path, "initial").expect("watched file should exist");
    let registration = registration(&watched_path, false, "n-file-watch");
    let spec = FileWatchSpec::from_registration(&registration).expect("spec should parse");
    let target = FileWatchTarget::from_spec(&registration, &spec).expect("target should resolve");
    let renamed_path = temporary_directory.path().join("renamed.txt");

    for (kind, expected) in [
        (EventKind::Create(CreateKind::File), "created"),
        (
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            "modified",
        ),
        (EventKind::Remove(RemoveKind::File), "deleted"),
    ] {
        let events = file_watch_events_from_notify_event(
            &registration,
            &target,
            &Event::new(kind).add_path(watched_path.clone()),
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["event"], expected);
    }

    let events = file_watch_events_from_notify_event(
        &registration,
        &target,
        &Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(watched_path)
            .add_path(renamed_path.clone()),
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["event"], "renamed");
    assert_path(&events[0], &renamed_path);
}

#[test]
fn watches_file_modify_delete_recreate_and_rename_lifecycle() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should exist");
    let watched_path = temporary_directory.path().join("watched.txt");
    let renamed_path = temporary_directory.path().join("renamed.txt");
    fs::write(&watched_path, "initial").expect("watched file should exist");
    let (service, receiver) = start_watcher(&watched_path, false, "n-file-watch");

    fs::write(&watched_path, "modified").expect("watched file should update");
    wait_for_event(&receiver, "modified", &watched_path, "n-file-watch");

    fs::remove_file(&watched_path).expect("watched file should delete");
    wait_for_event(&receiver, "deleted", &watched_path, "n-file-watch");

    fs::write(&watched_path, "recreated").expect("watched file should recreate");
    wait_for_event(&receiver, "created", &watched_path, "n-file-watch");

    fs::rename(&watched_path, &renamed_path).expect("watched file should rename");
    wait_for_event(&receiver, "renamed", &renamed_path, "n-file-watch");
    drop(service);
}

#[test]
fn recursive_setting_controls_nested_directory_events() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should exist");
    let nested_directory = temporary_directory.path().join("nested");
    fs::create_dir(&nested_directory).expect("nested directory should exist");

    let (non_recursive, receiver) = start_watcher(temporary_directory.path(), false, "n-flat");
    let root_file = temporary_directory.path().join("root.txt");
    fs::write(&root_file, "root").expect("root file should create");
    wait_for_path(&receiver, &root_file, "n-flat");
    let nested_file = nested_directory.join("ignored.txt");
    fs::write(&nested_file, "nested").expect("nested file should create");
    assert_no_path_event(&receiver, &nested_file, Duration::from_millis(400));
    drop(non_recursive);

    let (recursive, receiver) = start_watcher(temporary_directory.path(), true, "n-recursive");
    let recursive_file = nested_directory.join("included.txt");
    fs::write(&recursive_file, "nested").expect("recursive file should create");
    wait_for_path(&receiver, &recursive_file, "n-recursive");
    drop(recursive);
}

#[test]
fn rapid_directory_changes_are_delivered_without_path_loss() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should exist");
    let (service, receiver) = start_watcher(temporary_directory.path(), false, "n-rapid");
    let expected = (0..24)
        .map(|index| {
            temporary_directory
                .path()
                .join(format!("event-{index}.txt"))
        })
        .collect::<BTreeSet<_>>();
    for path in &expected {
        fs::write(path, "event").expect("rapid file should create");
    }

    let deadline = Instant::now() + EVENT_TIMEOUT;
    let mut observed = BTreeSet::new();
    while observed.len() < expected.len() && Instant::now() < deadline {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        if let Some(path) = event_path(&event)
            && expected.contains(&path)
        {
            observed.insert(path);
        }
    }
    assert_eq!(observed, expected);
    drop(service);
}

#[test]
fn replacing_watcher_registration_uses_only_the_new_node() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should exist");
    let watched_path = temporary_directory.path().join("watched.txt");
    fs::write(&watched_path, "initial").expect("watched file should exist");
    let (sender, receiver) = mpsc::sync_channel(64);
    let old = start_watcher_with_sender(&watched_path, false, "n-old", sender.clone());
    fs::write(&watched_path, "old").expect("old watcher write should succeed");
    wait_for_event(&receiver, "modified", &watched_path, "n-old");
    drop(old);
    while receiver.try_recv().is_ok() {}

    let new = start_watcher_with_sender(&watched_path, false, "n-new", sender);
    fs::write(&watched_path, "new").expect("new watcher write should succeed");
    let event = wait_for_event(&receiver, "modified", &watched_path, "n-new");
    assert_eq!(event.node_id, "n-new");
    drop(new);
}

#[test]
fn rejects_missing_watch_targets() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should exist");
    let missing = temporary_directory.path().join("missing.txt");
    let (sender, _receiver) = mpsc::sync_channel(1);
    let error = match FileWatchService::start([registration(&missing, false, "n-missing")], sender)
    {
        Ok(_) => panic!("missing watch target must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("is not accessible"), "{error}");
}

#[test]
fn rejects_runtime_templates_in_watch_paths() {
    let mut registration = registration(Path::new("watched.txt"), false, "n-template");
    registration.config["path"] = json!("{{runtime.watch_path}}");
    let error = FileWatchSpec::from_registration(&registration)
        .expect_err("runtime template path must fail");

    assert!(
        error
            .to_string()
            .contains("cannot use runtime variable templates"),
        "{error}"
    );
}

fn start_watcher(
    path: &Path,
    recursive: bool,
    node_id: &str,
) -> (FileWatchService, Receiver<TriggerEvent>) {
    let (sender, receiver) = mpsc::sync_channel(256);
    (
        start_watcher_with_sender(path, recursive, node_id, sender),
        receiver,
    )
}

fn start_watcher_with_sender(
    path: &Path,
    recursive: bool,
    node_id: &str,
    sender: SyncSender<TriggerEvent>,
) -> FileWatchService {
    FileWatchService::start([registration(path, recursive, node_id)], sender)
        .expect("file watcher should start")
}

fn wait_for_event(
    receiver: &Receiver<TriggerEvent>,
    event_name: &str,
    path: &Path,
    node_id: &str,
) -> TriggerEvent {
    let deadline = Instant::now() + EVENT_TIMEOUT;
    while Instant::now() < deadline {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        if event.node_id == node_id
            && event.payload["event"] == event_name
            && event_path(&event).as_deref() == Some(path)
        {
            return event;
        }
    }
    panic!(
        "did not receive {event_name} event for {} from {node_id}",
        path.display()
    );
}

fn wait_for_path(receiver: &Receiver<TriggerEvent>, path: &Path, node_id: &str) {
    let deadline = Instant::now() + EVENT_TIMEOUT;
    while Instant::now() < deadline {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        if event.node_id == node_id && event_path(&event).as_deref() == Some(path) {
            return;
        }
    }
    panic!(
        "did not receive event for {} from {node_id}",
        path.display()
    );
}

fn assert_no_path_event(receiver: &Receiver<TriggerEvent>, path: &Path, duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(50)) else {
            continue;
        };
        assert_ne!(event_path(&event).as_deref(), Some(path));
    }
}

fn event_path(event: &TriggerEvent) -> Option<PathBuf> {
    event.payload["path"].as_str().map(PathBuf::from)
}

fn assert_path(event: &TriggerEvent, path: &Path) {
    assert_eq!(event_path(event).as_deref(), Some(path));
}

fn registration(path: &Path, recursive: bool, node_id: &str) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.file_watch".to_owned(),
        config: json!({
            "path": path.to_string_lossy(),
            "recursive": recursive,
        }),
        node_id: node_id.to_owned(),
        runner_type: "file_watch".to_owned(),
        script_id: "script-file-watch".to_owned(),
        script_name: "File Watch Script".to_owned(),
    }
}
