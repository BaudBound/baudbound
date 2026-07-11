use std::{collections::BTreeMap, time::SystemTime};

use crate::TriggerEvent;

use super::{
    event::process_started_event,
    snapshot::{ProcessIdentity, ProcessSnapshot},
    spec::{ProcessMatchMode, ProcessStartedSpec, RegistrationId},
    state::ProcessTracker,
};

pub(super) struct ProcessStartedEngine {
    specs: BTreeMap<RegistrationId, ProcessStartedSpec>,
    tracker: ProcessTracker,
}

impl ProcessStartedEngine {
    pub(super) fn new(
        specs: BTreeMap<RegistrationId, ProcessStartedSpec>,
        baseline: &BTreeMap<ProcessIdentity, ProcessSnapshot>,
    ) -> Self {
        Self {
            specs,
            tracker: ProcessTracker::new(baseline.keys().copied()),
        }
    }

    pub(super) fn poll(
        &mut self,
        processes: &BTreeMap<ProcessIdentity, ProcessSnapshot>,
        detected_at: SystemTime,
        mut window_lookup: impl FnMut(u32, &str) -> Option<String>,
    ) -> Vec<TriggerEvent> {
        let new_processes = self.tracker.update(processes.keys().copied(), &self.specs);
        let mut events = Vec::new();

        for identity in new_processes {
            let Some(process) = processes.get(&identity) else {
                continue;
            };
            for spec in self
                .specs
                .values()
                .filter(|spec| spec.match_mode != ProcessMatchMode::WindowTitle)
            {
                if process_matches_spec(process, spec) {
                    events.push(process_started_event(spec, process, "", detected_at));
                }
            }
        }

        for (registration_id, identity) in self.tracker.pending_windows().collect::<Vec<_>>() {
            let Some(process) = processes.get(&identity) else {
                continue;
            };
            let Some(spec) = self.specs.get(&registration_id) else {
                continue;
            };
            if let Some(window_title) = window_lookup(identity.process_id, &spec.target) {
                events.push(process_started_event(
                    spec,
                    process,
                    &window_title,
                    detected_at,
                ));
                self.tracker.mark_window_matched(&registration_id, identity);
            }
        }
        events
    }

    pub(super) fn reconfigure_and_poll(
        &mut self,
        replacement: BTreeMap<RegistrationId, ProcessStartedSpec>,
        processes: &BTreeMap<ProcessIdentity, ProcessSnapshot>,
        detected_at: SystemTime,
        window_lookup: impl FnMut(u32, &str) -> Option<String>,
    ) -> Vec<TriggerEvent> {
        let unchanged = replacement
            .iter()
            .filter(|(id, spec)| self.specs.get(*id) == Some(*spec))
            .map(|(id, spec)| (id.clone(), spec.clone()))
            .collect();
        self.tracker.reconfigure(&self.specs, &replacement);
        self.specs = unchanged;
        let events = self.poll(processes, detected_at, window_lookup);
        self.specs = replacement;
        events
    }
}

fn process_matches_spec(process: &ProcessSnapshot, spec: &ProcessStartedSpec) -> bool {
    match spec.match_mode {
        ProcessMatchMode::ProcessName => process
            .process_name
            .to_string_lossy()
            .eq_ignore_ascii_case(spec.target.trim()),
        ProcessMatchMode::ExecutablePath => process
            .executable_path
            .as_ref()
            .map(|path| normalize_path(&path.display().to_string()))
            .is_some_and(|path| path == normalize_path(&spec.target)),
        ProcessMatchMode::WindowTitle => false,
    }
}

fn normalize_path(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}
