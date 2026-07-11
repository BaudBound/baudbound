use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant, SystemTime},
};

use serde_json::json;

use super::{
    engine::ProcessStartedEngine,
    service::ProcessStartedService,
    snapshot::{ProcessIdentity, ProcessSnapshot},
    spec::{ProcessMatchMode, ProcessStartedSpec, RegistrationId, collect_specs},
};
use crate::{TriggerEvent, TriggerRegistration};

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
const HELPER_ENVIRONMENT: &str = "BAUDBOUND_PROCESS_STARTED_TEST_HELPER";

#[test]
fn detects_each_process_identity_once_and_handles_pid_reuse() {
    let specs = collect_specs([registration("n-process", "process_name", "worker")])
        .expect("spec should parse");
    let baseline = snapshots([snapshot(10, 100, "existing", "existing")]);
    let mut engine = ProcessStartedEngine::new(specs, &baseline);
    let first = snapshots([
        snapshot(10, 100, "existing", "existing"),
        snapshot(20, 200, "worker", "worker"),
    ]);

    assert_eq!(poll(&mut engine, &first).len(), 1);
    assert!(poll(&mut engine, &first).is_empty());

    let reused_pid = snapshots([
        snapshot(10, 100, "existing", "existing"),
        snapshot(20, 201, "worker", "worker"),
    ]);
    let events = poll(&mut engine, &reused_pid);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["process_id"], 20);
}

#[test]
fn process_name_matching_uses_executable_name_when_system_name_is_truncated() {
    let executable_name = "baudbound-process-helper";
    let specs = collect_specs([registration("n-process", "process_name", executable_name)])
        .expect("spec should parse");
    let mut engine = ProcessStartedEngine::new(specs, &BTreeMap::new());
    let running = snapshots([snapshot(
        20,
        200,
        "baudbound-proces",
        &format!("/tmp/{executable_name}"),
    )]);

    let events = poll(&mut engine, &running);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node_id, "n-process");
}

#[test]
fn reconfigure_preserves_unchanged_window_candidates_and_baselines_changed_specs() {
    let original = specs([
        spec("n-window", ProcessMatchMode::WindowTitle, "Ready"),
        spec("n-old", ProcessMatchMode::ProcessName, "old"),
    ]);
    let baseline = snapshots([snapshot(10, 100, "existing", "existing")]);
    let mut engine = ProcessStartedEngine::new(original, &baseline);
    let running = snapshots([
        snapshot(10, 100, "existing", "existing"),
        snapshot(20, 200, "candidate", "candidate"),
    ]);
    assert!(poll(&mut engine, &running).is_empty());

    let replacement = specs([
        spec("n-window", ProcessMatchMode::WindowTitle, "Ready"),
        spec("n-new", ProcessMatchMode::ProcessName, "candidate"),
    ]);
    let events = engine.reconfigure_and_poll(
        replacement,
        &running,
        SystemTime::UNIX_EPOCH,
        |process_id, target| {
            (process_id == 20 && target == "Ready").then(|| "Ready Window".to_owned())
        },
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node_id, "n-window");
    assert!(poll(&mut engine, &running).is_empty());
}

#[test]
fn reload_boundary_notifies_only_unchanged_registrations_for_unobserved_processes() {
    let original = specs([
        spec("n-unchanged", ProcessMatchMode::ProcessName, "worker"),
        spec("n-removed", ProcessMatchMode::ProcessName, "worker"),
        spec("n-changed", ProcessMatchMode::ProcessName, "old"),
    ]);
    let baseline = snapshots([snapshot(10, 100, "existing", "existing")]);
    let mut engine = ProcessStartedEngine::new(original, &baseline);
    let replacement = specs([
        spec("n-unchanged", ProcessMatchMode::ProcessName, "worker"),
        spec("n-changed", ProcessMatchMode::ProcessName, "worker"),
        spec("n-added", ProcessMatchMode::ProcessName, "worker"),
    ]);
    let at_reload = snapshots([
        snapshot(10, 100, "existing", "existing"),
        snapshot(20, 200, "worker", "worker"),
    ]);

    let events =
        engine.reconfigure_and_poll(replacement, &at_reload, SystemTime::UNIX_EPOCH, |_, _| None);
    assert_eq!(
        events
            .iter()
            .map(|event| event.node_id.as_str())
            .collect::<Vec<_>>(),
        ["n-unchanged"]
    );
    assert!(poll(&mut engine, &at_reload).is_empty());

    let after_reload = snapshots([
        snapshot(10, 100, "existing", "existing"),
        snapshot(20, 200, "worker", "worker"),
        snapshot(30, 300, "worker", "worker"),
    ]);
    assert_eq!(poll(&mut engine, &after_reload).len(), 3);
}

#[test]
fn service_reconfiguration_reuses_the_worker_and_stops_promptly() {
    if std::env::var_os(HELPER_ENVIRONMENT).is_some() {
        std::thread::sleep(Duration::from_secs(2));
        return;
    }

    let source_executable = std::env::current_exe().expect("test executable should resolve");
    let temporary_directory = tempfile::tempdir().expect("temporary directory should exist");
    let helper_name = source_executable.extension().map_or_else(
        || format!("baudbound-process-helper-{}", std::process::id()),
        |extension| {
            format!(
                "baudbound-process-helper-{}.{}",
                std::process::id(),
                extension.to_string_lossy()
            )
        },
    );
    let executable = temporary_directory.path().join(helper_name);
    fs::copy(&source_executable, &executable).expect("test helper should be copied");
    let process_name = executable
        .file_name()
        .expect("test executable should have a name")
        .to_string_lossy()
        .into_owned();
    let (sender, receiver) = mpsc::sync_channel(32);
    let service = ProcessStartedService::start_with_interval(
        [registration("n-old", "process_name", &process_name)],
        sender.clone(),
        Duration::from_millis(25),
    )
    .expect("process watcher should start");

    let mut first_child = spawn_helper(&executable);
    wait_for_node(&receiver, "n-old");
    assert_no_node(&receiver, "n-old", Duration::from_millis(150));
    let _ = first_child.kill();
    let _ = first_child.wait();

    let service = ProcessStartedService::start_or_reconfigure(
        [registration("n-new", "process_name", &process_name)],
        sender,
        Some(service),
    )
    .expect("process watcher should reconfigure");
    assert_eq!(service.len(), 1);

    let mut second_child = spawn_helper(&executable);
    let event = wait_for_node(&receiver, "n-new");
    assert_eq!(event.script_id, "script-process");
    assert_no_node(&receiver, "n-old", Duration::from_millis(150));
    let _ = second_child.kill();
    let _ = second_child.wait();

    let started = Instant::now();
    drop(service);
    assert!(started.elapsed() < Duration::from_millis(500));
}

#[test]
fn rejects_duplicate_process_trigger_registrations() {
    let duplicate = registration("n-duplicate", "process_name", "worker");
    let error = collect_specs([duplicate.clone(), duplicate])
        .expect_err("duplicate registration must fail");
    assert!(
        error
            .to_string()
            .contains("duplicate process started trigger registration")
    );
}

fn poll(
    engine: &mut ProcessStartedEngine,
    processes: &BTreeMap<ProcessIdentity, ProcessSnapshot>,
) -> Vec<TriggerEvent> {
    engine.poll(processes, SystemTime::UNIX_EPOCH, |_, _| None)
}

fn snapshots(
    processes: impl IntoIterator<Item = ProcessSnapshot>,
) -> BTreeMap<ProcessIdentity, ProcessSnapshot> {
    processes
        .into_iter()
        .map(|process| (process.identity, process))
        .collect()
}

fn specs(
    specs: impl IntoIterator<Item = ProcessStartedSpec>,
) -> BTreeMap<RegistrationId, ProcessStartedSpec> {
    specs
        .into_iter()
        .map(|spec| (spec.id.clone(), spec))
        .collect()
}

fn spec(node_id: &str, match_mode: ProcessMatchMode, target: &str) -> ProcessStartedSpec {
    let registration = registration(node_id, "process_name", target);
    ProcessStartedSpec {
        id: RegistrationId {
            node_id: node_id.to_owned(),
            script_id: registration.script_id.clone(),
        },
        match_mode,
        registration,
        target: target.to_owned(),
    }
}

fn snapshot(process_id: u32, start_time: u64, name: &str, executable: &str) -> ProcessSnapshot {
    ProcessSnapshot {
        executable_path: Some(PathBuf::from(executable)),
        identity: ProcessIdentity {
            process_id,
            start_time,
        },
        process_name: OsString::from(name),
    }
}

fn spawn_helper(executable: &Path) -> Child {
    Command::new(executable)
        .args([
            "--exact",
            "services::process_started::tests::service_reconfiguration_reuses_the_worker_and_stops_promptly",
            "--nocapture",
        ])
        .env(HELPER_ENVIRONMENT, "1")
        .spawn()
        .expect("process helper should start")
}

fn wait_for_node(receiver: &Receiver<TriggerEvent>, node_id: &str) -> TriggerEvent {
    let deadline = Instant::now() + EVENT_TIMEOUT;
    while Instant::now() < deadline {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(50)) else {
            continue;
        };
        if event.node_id == node_id {
            return event;
        }
    }
    panic!("did not receive process event for {node_id}");
}

fn assert_no_node(receiver: &Receiver<TriggerEvent>, node_id: &str, duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(25)) else {
            continue;
        };
        assert_ne!(event.node_id, node_id);
    }
}

fn registration(node_id: &str, match_mode: &str, target: &str) -> TriggerRegistration {
    TriggerRegistration {
        action_type: "trigger.process_started".to_owned(),
        config: json!({
            "matchMode": match_mode,
            "target": target,
        }),
        node_id: node_id.to_owned(),
        runner_type: "process_started".to_owned(),
        script_id: "script-process".to_owned(),
        script_name: "Process Script".to_owned(),
    }
}
