use std::collections::{BTreeMap, BTreeSet};

use super::{
    snapshot::ProcessIdentity,
    spec::{ProcessMatchMode, ProcessStartedSpec, RegistrationId},
};

pub(super) struct ProcessTracker {
    known_processes: BTreeSet<ProcessIdentity>,
    pending_windows: BTreeSet<(RegistrationId, ProcessIdentity)>,
}

impl ProcessTracker {
    pub(super) fn new(initial_processes: impl IntoIterator<Item = ProcessIdentity>) -> Self {
        Self {
            known_processes: initial_processes.into_iter().collect(),
            pending_windows: BTreeSet::new(),
        }
    }

    pub(super) fn update(
        &mut self,
        current_processes: impl IntoIterator<Item = ProcessIdentity>,
        specs: &BTreeMap<RegistrationId, ProcessStartedSpec>,
    ) -> Vec<ProcessIdentity> {
        let current = current_processes.into_iter().collect::<BTreeSet<_>>();
        let new_processes = current
            .difference(&self.known_processes)
            .copied()
            .collect::<Vec<_>>();

        self.pending_windows
            .retain(|(_, process)| current.contains(process));
        for process in &new_processes {
            for spec in specs
                .values()
                .filter(|spec| spec.match_mode == ProcessMatchMode::WindowTitle)
            {
                self.pending_windows.insert((spec.id.clone(), *process));
            }
        }
        self.known_processes = current;
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
