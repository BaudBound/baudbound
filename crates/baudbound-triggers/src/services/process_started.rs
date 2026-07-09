use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

use serde_json::{Value, json};
use sysinfo::{ProcessesToUpdate, System};

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics,
    unix_timestamp_millis,
};

pub struct ProcessStartedService {
    handles: Vec<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl ProcessStartedService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            handles: Vec::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: Sender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let specs = registrations
            .into_iter()
            .filter(|registration| registration.action_type == "trigger.process_started")
            .map(ProcessStartedSpec::from_registration)
            .collect::<Result<Vec<_>, _>>()?;

        if specs.is_empty() {
            return Ok(Self::empty());
        }

        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let handle = thread::spawn(move || {
            run_process_started_watcher(specs, sender, thread_running);
        });

        Ok(Self {
            handles: vec![handle],
            running,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::thread_backed(
            self.running.load(Ordering::Relaxed),
            self.len(),
            "process watcher thread",
        )
    }
}

impl Drop for ProcessStartedService {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessStartedSpec {
    pub(crate) match_mode: String,
    registration: TriggerRegistration,
    pub(crate) target: String,
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
        let match_mode = registration
            .config
            .get("matchMode")
            .and_then(Value::as_str)
            .unwrap_or("process_name")
            .trim()
            .to_owned();
        if !matches!(
            match_mode.as_str(),
            "process_name" | "executable_path" | "window_title"
        ) {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                format!("unsupported process started match mode {match_mode:?}"),
            ));
        }
        if match_mode == "window_title" {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                "window title process start matching requires the desktop runner".to_owned(),
            ));
        }

        Ok(Self {
            match_mode,
            registration,
            target,
        })
    }
}

fn run_process_started_watcher(
    specs: Vec<ProcessStartedSpec>,
    sender: Sender<TriggerEvent>,
    running: Arc<AtomicBool>,
) {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut seen_processes = system.processes().keys().copied().collect::<Vec<_>>();
    seen_processes.sort_by_key(|pid| pid.as_u32());

    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_secs(1));
        system.refresh_processes(ProcessesToUpdate::All, true);

        let mut current_processes = system.processes().keys().copied().collect::<Vec<_>>();
        current_processes.sort_by_key(|pid| pid.as_u32());

        for pid in current_processes.iter().copied().filter(|pid| {
            seen_processes
                .binary_search_by_key(&pid.as_u32(), |seen_pid| seen_pid.as_u32())
                .is_err()
        }) {
            let Some(process) = system.process(pid) else {
                continue;
            };
            for spec in &specs {
                if process_matches_spec(process, spec) {
                    let _ = sender.send(process_started_event(spec, process));
                }
            }
        }

        seen_processes = current_processes;
    }
}

fn process_matches_spec(process: &sysinfo::Process, spec: &ProcessStartedSpec) -> bool {
    match spec.match_mode.as_str() {
        "process_name" => process
            .name()
            .to_string_lossy()
            .eq_ignore_ascii_case(spec.target.trim()),
        "executable_path" => process
            .exe()
            .map(|path| normalize_path_string(&path.display().to_string()))
            .is_some_and(|path| path == normalize_path_string(&spec.target)),
        _ => false,
    }
}

fn process_started_event(spec: &ProcessStartedSpec, process: &sysinfo::Process) -> TriggerEvent {
    TriggerEvent {
        node_id: spec.registration.node_id.clone(),
        payload: json!({
            "executable_path": process.exe().map(|path| path.display().to_string()).unwrap_or_default(),
            "process_id": process.pid().as_u32(),
            "process_name": process.name().to_string_lossy(),
            "timestamp": unix_timestamp_millis(SystemTime::now()).to_string(),
            "window_title": "",
        }),
        script_id: spec.registration.script_id.clone(),
    }
}

fn normalize_path_string(path: &str) -> String {
    path.trim().replace('\\', "/").to_ascii_lowercase()
}
