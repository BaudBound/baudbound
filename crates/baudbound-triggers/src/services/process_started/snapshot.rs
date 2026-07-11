use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

use sysinfo::{ProcessesToUpdate, System};

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ProcessIdentity {
    pub(super) process_id: u32,
    pub(super) start_time: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ProcessSnapshot {
    pub(super) executable_path: Option<PathBuf>,
    pub(super) identity: ProcessIdentity,
    pub(super) process_name: OsString,
}

pub(super) struct ProcessSource {
    system: System,
}

impl ProcessSource {
    pub(super) fn new() -> Self {
        Self {
            system: System::new(),
        }
    }

    pub(super) fn refresh(&mut self) -> BTreeMap<ProcessIdentity, ProcessSnapshot> {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.system
            .processes()
            .values()
            .map(ProcessSnapshot::from_process)
            .map(|snapshot| (snapshot.identity, snapshot))
            .collect()
    }
}

impl ProcessSnapshot {
    fn from_process(process: &sysinfo::Process) -> Self {
        Self {
            executable_path: process.exe().map(PathBuf::from),
            identity: ProcessIdentity {
                process_id: process.pid().as_u32(),
                start_time: process.start_time(),
            },
            process_name: process.name().to_owned(),
        }
    }
}
