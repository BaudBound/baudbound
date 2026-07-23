use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result};

pub(super) fn install_shutdown_handler() -> Result<Arc<AtomicBool>> {
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let handler_shutdown_requested = Arc::clone(&shutdown_requested);
    ctrlc::set_handler(move || {
        handler_shutdown_requested.store(true, Ordering::SeqCst);
    })
    .context("failed to install Ctrl+C shutdown handler")?;

    Ok(shutdown_requested)
}
