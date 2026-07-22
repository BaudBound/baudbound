use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{SyncSender, TrySendError, sync_channel},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use baudbound_core::TriggerEvent;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime};

const CHANNEL_CAPACITY: usize = 256;
const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
const MAIN_WINDOW_LABEL: &str = "main";
pub(crate) const TRIGGER_MONITOR_EVENT_CHANNEL: &str = "runner-trigger-monitor";

#[derive(Clone, Default)]
pub(crate) struct TriggerMonitor {
    inner: Arc<TriggerMonitorInner>,
}

#[derive(Default)]
struct TriggerMonitorInner {
    enabled: AtomicBool,
    omitted_events: AtomicUsize,
    sender: Mutex<Option<SyncSender<TriggerMonitorEvent>>>,
    sequence: AtomicU64,
    session_id: AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TriggerMonitorStatus {
    Queued,
    Rejected,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TriggerMonitorEvent {
    pub(crate) action_type: String,
    pub(crate) error: Option<String>,
    pub(crate) node_id: String,
    pub(crate) omitted_event_count: usize,
    pub(crate) payload_bytes: usize,
    pub(crate) payload_json: String,
    pub(crate) payload_truncated: bool,
    pub(crate) script_id: String,
    pub(crate) sequence: u64,
    pub(crate) session_id: u64,
    pub(crate) source: String,
    pub(crate) status: TriggerMonitorStatus,
    pub(crate) timestamp_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct TriggerMonitorState {
    enabled: bool,
    omitted_event_count: usize,
    session_id: u64,
}

pub(crate) trait TriggerMonitorEventSink: Send + Sync {
    fn publish(&self, event: TriggerMonitorEvent);
}

struct TauriTriggerMonitorEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TriggerMonitorEventSink for TauriTriggerMonitorEventSink<R> {
    fn publish(&self, event: TriggerMonitorEvent) {
        let _ = self
            .app
            .emit_to(MAIN_WINDOW_LABEL, TRIGGER_MONITOR_EVENT_CHANNEL, event);
    }
}

impl TriggerMonitor {
    pub(crate) fn connect_event_sink<R: Runtime>(&self, app: AppHandle<R>) -> Result<(), String> {
        self.connect_sink(Arc::new(TauriTriggerMonitorEventSink { app }))
    }

    pub(crate) fn connect_sink(
        &self,
        sink: Arc<dyn TriggerMonitorEventSink>,
    ) -> Result<(), String> {
        let (sender, receiver) = sync_channel::<TriggerMonitorEvent>(CHANNEL_CAPACITY);
        let mut current_sender = self
            .inner
            .sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if current_sender.is_some() {
            return Ok(());
        }

        let inner = Arc::downgrade(&self.inner);
        thread::Builder::new()
            .name("baudbound-trigger-monitor".to_owned())
            .spawn(move || {
                while let Ok(mut event) = receiver.recv() {
                    let Some(inner) = Weak::upgrade(&inner) else {
                        return;
                    };
                    if !inner.enabled.load(Ordering::Acquire)
                        || event.session_id != inner.session_id.load(Ordering::Acquire)
                    {
                        continue;
                    }
                    event.omitted_event_count = inner.omitted_events.swap(0, Ordering::AcqRel);
                    sink.publish(event);
                }
            })
            .map_err(|source| format!("failed to start trigger monitor event worker: {source}"))?;
        *current_sender = Some(sender);
        Ok(())
    }

    pub(crate) fn start(&self) -> TriggerMonitorState {
        let session_id = self.next_session();
        self.inner.sequence.store(0, Ordering::Release);
        self.inner.omitted_events.store(0, Ordering::Release);
        self.inner.enabled.store(true, Ordering::Release);
        TriggerMonitorState {
            enabled: true,
            omitted_event_count: 0,
            session_id,
        }
    }

    pub(crate) fn stop(&self) -> TriggerMonitorState {
        self.inner.enabled.store(false, Ordering::Release);
        let session_id = self.next_session();
        let omitted_event_count = self.inner.omitted_events.swap(0, Ordering::AcqRel);
        TriggerMonitorState {
            enabled: false,
            omitted_event_count,
            session_id,
        }
    }

    pub(crate) fn clear(&self) -> TriggerMonitorState {
        let enabled = self.inner.enabled.load(Ordering::Acquire);
        let session_id = self.next_session();
        self.inner.sequence.store(0, Ordering::Release);
        self.inner.omitted_events.store(0, Ordering::Release);
        TriggerMonitorState {
            enabled,
            omitted_event_count: 0,
            session_id,
        }
    }

    pub(crate) fn state(&self) -> TriggerMonitorState {
        TriggerMonitorState {
            enabled: self.inner.enabled.load(Ordering::Acquire),
            omitted_event_count: self.inner.omitted_events.load(Ordering::Acquire),
            session_id: self.inner.session_id.load(Ordering::Acquire),
        }
    }

    pub(crate) fn observe_submission(
        &self,
        event: &TriggerEvent,
        source: &str,
        status: TriggerMonitorStatus,
        error: Option<&str>,
    ) {
        if !self.inner.enabled.load(Ordering::Acquire) {
            return;
        }
        let session_id = self.inner.session_id.load(Ordering::Acquire);
        let sequence = self.inner.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let (payload_json, payload_bytes, payload_truncated) = payload_document(&event.payload);
        let monitor_event = TriggerMonitorEvent {
            action_type: event.action_type.clone(),
            error: error.map(ToOwned::to_owned),
            node_id: event.node_id.clone(),
            omitted_event_count: 0,
            payload_bytes,
            payload_json,
            payload_truncated,
            script_id: event.script_id.clone(),
            sequence,
            session_id,
            source: source.to_owned(),
            status,
            timestamp_unix_ms: unix_timestamp_millis(SystemTime::now()),
        };

        let sender = self
            .inner
            .sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let Some(sender) = sender else {
            self.inner.omitted_events.fetch_add(1, Ordering::Relaxed);
            return;
        };
        match sender.try_send(monitor_event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.inner.omitted_events.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn next_session(&self) -> u64 {
        self.inner.session_id.fetch_add(1, Ordering::AcqRel) + 1
    }
}

fn payload_document(payload: &Value) -> (String, usize, bool) {
    let sanitized = sanitize_payload(payload, None);
    let serialized = serde_json::to_string(&sanitized).unwrap_or_else(|_| "null".to_owned());
    let payload_bytes = serialized.len();
    if payload_bytes <= MAX_PAYLOAD_BYTES {
        return (serialized, payload_bytes, false);
    }
    let mut boundary = MAX_PAYLOAD_BYTES;
    while !serialized.is_char_boundary(boundary) {
        boundary -= 1;
    }
    (serialized[..boundary].to_owned(), payload_bytes, true)
}

fn sanitize_payload(value: &Value, parent_key: Option<&str>) -> Value {
    match value {
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| sanitize_payload(value, parent_key))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| {
                    let redact = parent_key
                        .is_some_and(|parent| parent.eq_ignore_ascii_case("headers"))
                        && is_sensitive_header(key);
                    (
                        key.clone(),
                        if redact {
                            Value::String("[redacted]".to_owned())
                        } else {
                            sanitize_payload(value, Some(key))
                        },
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization"
            | "cookie"
            | "proxy-authorization"
            | "set-cookie"
            | "x-api-key"
            | "x-baudbound-token"
    )
}

fn unix_timestamp_millis(timestamp: SystemTime) -> u64 {
    timestamp
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct CollectingSink(Mutex<Vec<TriggerMonitorEvent>>);

    impl TriggerMonitorEventSink for CollectingSink {
        fn publish(&self, event: TriggerMonitorEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    #[test]
    fn disabled_monitor_does_not_publish_events() {
        let monitor = TriggerMonitor::default();
        let sink = Arc::new(CollectingSink::default());
        monitor.connect_sink(sink.clone()).unwrap();
        monitor.observe_submission(
            &event(Value::Null),
            "manual",
            TriggerMonitorStatus::Queued,
            None,
        );
        thread::sleep(std::time::Duration::from_millis(10));
        assert!(sink.0.lock().unwrap().is_empty());
    }

    #[test]
    fn monitor_redacts_sensitive_network_headers() {
        let monitor = TriggerMonitor::default();
        let sink = Arc::new(CollectingSink::default());
        monitor.connect_sink(sink.clone()).unwrap();
        monitor.start();
        monitor.observe_submission(
            &event(serde_json::json!({
                "headers": {
                    "authorization": "Bearer secret",
                    "x-client": "visible"
                }
            })),
            "webhook",
            TriggerMonitorStatus::Queued,
            None,
        );
        for _ in 0..20 {
            if !sink.0.lock().unwrap().is_empty() {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        let events = sink.0.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].payload_json.contains("[redacted]"));
        assert!(!events[0].payload_json.contains("Bearer secret"));
        assert!(events[0].payload_json.contains("visible"));
    }

    fn event(payload: Value) -> TriggerEvent {
        TriggerEvent {
            action_type: "trigger.webhook".to_owned(),
            node_id: "node-1".to_owned(),
            payload,
            script_id: "script-1".to_owned(),
        }
    }
}
