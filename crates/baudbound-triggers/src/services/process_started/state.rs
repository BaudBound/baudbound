use std::collections::{BTreeMap, BTreeSet};

use super::{
    snapshot::ProcessIdentity,
    spec::{ProcessMatchMode, ProcessStartedSpec, RegistrationId},
};

pub(super) struct ProcessTracker {
    known_processes: BTreeMap<u32, u64>,
    pending_windows: BTreeSet<(RegistrationId, ProcessIdentity)>,
}

impl ProcessTracker {
    pub(super) fn new(initial_processes: impl IntoIterator<Item = ProcessIdentity>) -> Self {
        Self {
            known_processes: initial_processes
                .into_iter()
                .map(|process| (process.process_id, process.start_time))
                .collect(),
            pending_windows: BTreeSet::new(),
        }
    }

    pub(super) fn update(
        &mut self,
        current_processes: impl IntoIterator<Item = ProcessIdentity>,
        specs: &BTreeMap<RegistrationId, ProcessStartedSpec>,
    ) -> Vec<ProcessIdentity> {
        let current = current_processes
            .into_iter()
            .map(|process| (process.process_id, process))
            .collect::<BTreeMap<_, _>>();
        let new_processes = current
            .values()
            .filter(|process| {
                self.known_processes
                    .get(&process.process_id)
                    .is_none_or(|known_start| {
                        !start_times_equivalent(*known_start, process.start_time)
                    })
            })
            .copied()
            .collect::<Vec<_>>();

        self.pending_windows = self
            .pending_windows
            .iter()
            .filter_map(|(registration_id, pending)| {
                current
                    .get(&pending.process_id)
                    .filter(|current| {
                        start_times_equivalent(pending.start_time, current.start_time)
                    })
                    .map(|current| (registration_id.clone(), *current))
            })
            .collect();
        for process in &new_processes {
            for spec in specs
                .values()
                .filter(|spec| spec.match_mode == ProcessMatchMode::WindowTitle)
            {
                self.pending_windows.insert((spec.id.clone(), *process));
            }
        }
        self.known_processes = current
            .into_iter()
            .map(|(process_id, process)| (process_id, process.start_time))
            .collect();
        new_processes
    }

    pub(super) fn reconfigure(
        &mut self,
        previous: &BTreeMap<RegistrationId, ProcessStartedSpec>,
        replacement: &BTreeMap<RegistrationId, ProcessStartedSpec>,
    ) {
        self.pending_windows.retain(|(registration_id, _)| {
            previous.get(registration_id) == replacement.get(registration_id)
        });
    }

    pub(super) fn pending_windows(
        &self,
    ) -> impl Iterator<Item = (RegistrationId, ProcessIdentity)> + '_ {
        self.pending_windows.iter().cloned()
    }

    pub(super) fn mark_window_matched(
        &mut self,
        registration_id: &RegistrationId,
        process: ProcessIdentity,
    ) {
        self.pending_windows
            .remove(&(registration_id.clone(), process));
    }
}

fn start_times_equivalent(left: u64, right: u64) -> bool {
    left == right || left == 0 || right == 0
}
