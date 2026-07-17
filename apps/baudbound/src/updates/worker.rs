use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use baudbound_core::UpdateSettings;
use baudbound_storage::SqliteRunnerStore;

use super::{check_is_due, check_now};

const MAX_SCHEDULE_POLL_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub struct AutomaticUpdateWorker {
    shutdown: Arc<(Mutex<bool>, Condvar)>,
}

impl AutomaticUpdateWorker {
    pub fn start(store: SqliteRunnerStore, settings: UpdateSettings) -> Option<Self> {
        if !settings.automatic_checks {
            return None;
        }

        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_shutdown = Arc::clone(&shutdown);
        if let Err(error) = thread::Builder::new()
            .name("baudbound-update-check".to_owned())
            .spawn(move || run_worker(&store, &settings, &worker_shutdown))
        {
            tracing::warn!(%error, "failed to start automatic update checker");
            return None;
        }

        Some(Self { shutdown })
    }
}

impl Drop for AutomaticUpdateWorker {
    fn drop(&mut self) {
        let (shutdown, wake) = &*self.shutdown;
        let mut requested = shutdown.lock().unwrap_or_else(|error| error.into_inner());
        *requested = true;
        wake.notify_all();
    }
}

fn run_worker(
    store: &SqliteRunnerStore,
    settings: &UpdateSettings,
    shutdown: &Arc<(Mutex<bool>, Condvar)>,
) {
    let configured_interval =
        Duration::from_secs(settings.check_interval_hours.saturating_mul(60 * 60));
    let poll_interval = configured_interval
        .min(MAX_SCHEDULE_POLL_INTERVAL)
        .max(Duration::from_secs(60));

    loop {
        match check_is_due(store, settings.check_interval_hours) {
            Ok(true) => match check_now(store) {
                Ok(result) if result.update_available => tracing::info!(
                    latest_version = %result.latest_version,
                    "a BaudBound update is available"
                ),
                Ok(_) => tracing::debug!("automatic update check completed"),
                Err(error) => tracing::debug!(%error, "automatic update check failed"),
            },
            Ok(false) => {}
            Err(error) => tracing::debug!(%error, "failed to inspect update check schedule"),
        }

        let (requested, wake) = &**shutdown;
        let requested = requested.lock().unwrap_or_else(|error| error.into_inner());
        if *requested {
            return;
        }
        let (requested, _) = wake
            .wait_timeout(requested, poll_interval)
            .unwrap_or_else(|error| error.into_inner());
        if *requested {
            return;
        }
    }
}
