use std::{
    io::{self, BufRead},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
};

use anyhow::{Context, Result};
use baudbound_storage::{ConsumedServiceControl, FilesystemScriptStore, ServiceControlCommand};

pub(super) fn consume_service_control_request(
    store: &FilesystemScriptStore,
) -> Result<Option<ServiceControlCommand>> {
    match store
        .consume_service_control_request_for_pid(std::process::id())
        .context("failed to read service control request")?
    {
        Some(ConsumedServiceControl::Command(command)) => Ok(Some(command)),
        Some(ConsumedServiceControl::Ignored { reason }) => {
            eprintln!("Ignoring service control request: {reason}.");
            Ok(None)
        }
        None => Ok(None),
    }
}

pub(super) fn install_shutdown_handler() -> Result<Arc<AtomicBool>> {
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let handler_shutdown_requested = Arc::clone(&shutdown_requested);
    ctrlc::set_handler(move || {
        handler_shutdown_requested.store(true, Ordering::SeqCst);
    })
    .context("failed to install Ctrl+C shutdown handler")?;

    Ok(shutdown_requested)
}

pub(super) fn spawn_hotkey_stdin_reader(sender: Sender<String>) {
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(key) = line else {
                break;
            };
            let key = key.trim().to_owned();
            if key.is_empty() {
                continue;
            }
            if sender.send(key).is_err() {
                break;
            }
        }
    });
}
