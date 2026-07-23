use serde_json::{Value, json};

use crate::paths::current_unix_timestamp;

const DEFAULT_STALE_AFTER_SECONDS: u64 = 15;
const RELOAD_INTERVAL_STALE_MULTIPLIER: u64 = 3;

pub fn service_health_document(service_status: Option<&Value>) -> Value {
    let now_unix = current_unix_timestamp();
    let Some(service_status) = service_status else {
        return json!({
            "health": "missing",
            "ok": false,
            "reason": "No service status has been written yet.",
            "stale": false,
        });
    };

    let state = service_status
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let last_heartbeat_unix = service_status
        .get("last_heartbeat_unix")
        .and_then(Value::as_u64);
    let reload_interval_seconds = service_status
        .get("reload_interval_seconds")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let stale_after_seconds = stale_after_seconds(reload_interval_seconds);
    let heartbeat_age_seconds =
        last_heartbeat_unix.map(|last_heartbeat| now_unix.saturating_sub(last_heartbeat));

    if state != "running" {
        return json!({
            "health": state,
            "heartbeat_age_seconds": heartbeat_age_seconds,
            "ok": true,
            "reason": format!("Service state is {state}."),
            "stale": false,
            "stale_after_seconds": stale_after_seconds,
        });
    }

    let Some(heartbeat_age_seconds) = heartbeat_age_seconds else {
        return json!({
            "health": "unknown",
            "ok": false,
            "reason": "Running service status does not contain last_heartbeat_unix.",
            "stale": true,
            "stale_after_seconds": stale_after_seconds,
        });
    };
    let stale = heartbeat_age_seconds > stale_after_seconds;

    json!({
        "health": if stale { "stale" } else { "healthy" },
        "heartbeat_age_seconds": heartbeat_age_seconds,
        "ok": !stale,
        "reason": format!("Last heartbeat is {heartbeat_age_seconds}s old."),
        "stale": stale,
        "stale_after_seconds": stale_after_seconds,
    })
}

fn stale_after_seconds(reload_interval_seconds: u64) -> u64 {
    DEFAULT_STALE_AFTER_SECONDS
        .max(reload_interval_seconds.saturating_mul(RELOAD_INTERVAL_STALE_MULTIPLIER))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_missing_status_as_not_ok() {
        let health = service_health_document(None);

        assert_eq!(health["health"], "missing");
        assert_eq!(health["ok"], false);
    }

    #[test]
    fn marks_old_running_heartbeat_as_stale() {
        let health = service_health_document(Some(&json!({
            "last_heartbeat_unix": 1,
            "reload_interval_seconds": 2,
            "state": "running"
        })));

        assert_eq!(health["health"], "stale");
        assert_eq!(health["stale"], true);
        assert_eq!(health["stale_after_seconds"], 15);
    }

    #[test]
    fn stopped_service_is_not_stale() {
        let health = service_health_document(Some(&json!({
            "last_heartbeat_unix": 1,
            "state": "stopped"
        })));

        assert_eq!(health["health"], "stopped");
        assert_eq!(health["ok"], true);
        assert_eq!(health["stale"], false);
    }
}
