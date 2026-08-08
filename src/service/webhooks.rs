use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use baudbound_core::{RunnerCore, TriggerRegistration};
use baudbound_runtime::RuntimeCancellationToken;
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::{
    NetworkTriggerAuthenticator, NetworkTriggerKind, WebhookDispatch, WebhookResponse,
    WebhookService,
};

use crate::console;

use super::{
    executor::{TriggerCompletion, TriggerExecutor, TriggerSubmitError},
    heartbeat::ServeStatusTracker,
    network_auth::{
        ListenerExposurePolicy, RunnerNetworkTriggerAuthenticator, validate_listener_exposure,
    },
    options::ServeOptions,
};

mod http;
mod listener;

use http::with_cors_origin;
use listener::{IncomingWebhook, WebhookListener, WebhookListenerConfig, WebhookResponseSender};

const COMPLETION_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) struct WebhookHost {
    executor: TriggerExecutor,
    listener: WebhookListener,
    pending: BTreeMap<u64, PendingWebhookResponse>,
    pub(super) service: WebhookService,
}

struct PendingWebhookResponse {
    cors_origin: Option<String>,
    deadline: Instant,
    dispatch: WebhookDispatch,
    response: WebhookResponseSender,
}

impl WebhookHost {
    pub(super) fn has_pending_execution(&self) -> bool {
        self.executor.has_pending()
    }

    pub(super) fn accept_next(&mut self, wait_duration: Duration) -> Result<()> {
        let Some(incoming) = self
            .listener
            .recv_timeout(wait_duration)
            .map_err(|_| anyhow!("webhook listener channel disconnected"))?
        else {
            return Ok(());
        };
        self.accept_request(incoming);
        Ok(())
    }

    fn accept_request(&mut self, incoming: IncomingWebhook) {
        let IncomingWebhook {
            dispatch,
            origin,
            response,
        } = incoming;

        console::info(format_args!(
            "Queueing webhook trigger {} for script {}",
            dispatch.event.node_id, dispatch.event.script_id
        ));
        let job_id = match self.executor.submit_from(dispatch.event.clone(), "webhook") {
            Ok(job_id) => job_id,
            Err(TriggerSubmitError::Full) => {
                response.send(overloaded_response());
                return;
            }
            Err(TriggerSubmitError::Stopped) => {
                response.send(unavailable_response());
                return;
            }
        };

        if dispatch.wait_for_response {
            self.pending.insert(
                job_id,
                PendingWebhookResponse {
                    cors_origin: origin,
                    deadline: Instant::now() + dispatch.response_timeout,
                    dispatch,
                    response,
                },
            );
        } else {
            response.send(with_cors_origin(
                dispatch.fallback_response,
                origin.as_deref(),
            ));
        }
    }

    pub(super) fn poll(&mut self, status: &mut ServeStatusTracker) -> bool {
        let mut completed_any = false;
        while let Some(completion) = self.executor.try_completion() {
            completed_any = true;
            self.record_completion(status, completion);
        }
        self.expire_pending();
        completed_any
    }

    pub(super) fn response_poll_interval(&self) -> Option<Duration> {
        if self.pending.is_empty() {
            return None;
        }
        let now = Instant::now();
        let until_deadline = self
            .pending
            .values()
            .map(|pending| pending.deadline.saturating_duration_since(now))
            .min()
            .unwrap_or(COMPLETION_POLL_INTERVAL);
        Some(until_deadline.min(COMPLETION_POLL_INTERVAL))
    }

    fn record_completion(
        &mut self,
        status: &mut ServeStatusTracker,
        completion: TriggerCompletion,
    ) {
        match completion.result {
            Ok(baudbound_core::TriggerActivation::Started { report }) => {
                status.record_report("webhook", &report);
                if let Some(pending) = self.pending.remove(&completion.job_id) {
                    let response = self.service.response_for_report(&pending.dispatch, &report);
                    pending.response.send(with_cors_origin(
                        with_outcome_header(response, "started"),
                        pending.cors_origin.as_deref(),
                    ));
                }
            }
            // Nothing ran, so the configured response never happens. Answer
            // 202 rather than the author configured status, which is theirs to
            // choose and could be anything, and name the outcome in a header a
            // caller can branch on without parsing the body.
            Ok(outcome) => {
                if let Some(pending) = self.pending.remove(&completion.job_id) {
                    let name = outcome.outcome_name();
                    let body = match &outcome {
                        baudbound_core::TriggerActivation::Stopped { cancelled } => {
                            format!("Stopped {cancelled} running instance(s). No new run started.")
                        }
                        _ => "Skipped: the script is already running.".to_owned(),
                    };
                    pending.response.send(with_cors_origin(
                        with_outcome_header(text_response(202, body), name),
                        pending.cors_origin.as_deref(),
                    ));
                }
            }
            Err(error) => {
                status.record_event_failure("webhook", &completion.event, error.clone());
                if let Some(pending) = self.pending.remove(&completion.job_id) {
                    pending.response.send(with_cors_origin(
                        dispatch_failed_response(&error),
                        pending.cors_origin.as_deref(),
                    ));
                }
            }
        }
    }

    fn expire_pending(&mut self) {
        let now = Instant::now();
        let expired = self
            .pending
            .iter()
            .filter_map(|(job_id, pending)| (pending.deadline <= now).then_some(*job_id))
            .collect::<Vec<_>>();
        for job_id in expired {
            if let Some(pending) = self.pending.remove(&job_id) {
                pending.response.send(with_cors_origin(
                    pending.dispatch.fallback_response,
                    pending.cors_origin.as_deref(),
                ));
            }
        }
    }
}

impl Drop for WebhookHost {
    fn drop(&mut self) {
        for (_, pending) in std::mem::take(&mut self.pending) {
            pending.response.send(unavailable_response());
        }
    }
}

pub(super) fn build_webhook_host(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    registrations: Vec<TriggerRegistration>,
    options: &ServeOptions,
    previous_webhook_host: Option<WebhookHost>,
    cancellation: &RuntimeCancellationToken,
) -> Result<Option<WebhookHost>> {
    if !options.webhooks_enabled {
        return Ok(None);
    }

    validate_listener_exposure(
        core,
        store,
        &registrations,
        NetworkTriggerKind::Webhook,
        &options.webhook_bind,
        options.webhook_port,
        ListenerExposurePolicy {
            allow_public_network_listeners: options.allow_public_network_listeners,
            allow_unauthenticated_public_bind: options.webhook_allow_unauthenticated_public_bind,
        },
    )?;

    let service = WebhookService::from_registrations(registrations)
        .context("failed to register webhook triggers")?;
    if service.is_empty() {
        console::info(format_args!("No enabled webhook triggers found."));
        return Ok(None);
    }

    let address = format!("{}:{}", options.webhook_bind, options.webhook_port);
    let listener_config = WebhookListenerConfig {
        bind_address: address.clone(),
        body_read_progress_timeout_ms: options.webhook_body_read_progress_timeout_ms,
        body_read_timeout_ms: options.webhook_body_read_timeout_ms,
        header_read_timeout_ms: options.webhook_header_read_timeout_ms,
        max_body_bytes: options.max_webhook_body_bytes,
        max_connections: options.max_webhook_connections,
        max_header_bytes: options.webhook_max_header_bytes,
        max_unauthenticated_connections: options.webhook_max_unauthenticated_connections,
        pre_auth_requests_per_minute_global: options.webhook_pre_auth_requests_per_minute_global,
        pre_auth_requests_per_minute_per_address: options
            .webhook_pre_auth_requests_per_minute_per_address,
        pre_auth_timeout_ms: options.webhook_pre_auth_timeout_ms,
    };
    let authenticator: Arc<dyn NetworkTriggerAuthenticator> =
        Arc::new(RunnerNetworkTriggerAuthenticator::new(core, store));
    if let Some(mut host) = previous_webhook_host {
        let listener_restarted = !host.listener.matches_configuration(&listener_config);
        host.listener
            .restart(
                listener_config,
                service.clone(),
                Arc::clone(&authenticator),
                options.webhook_allow_browser_origins.clone(),
            )
            .map_err(|error| anyhow!("failed to reload webhook listener on {address}: {error}"))?;
        if listener_restarted {
            console::info(format_args!(
                "Reloaded webhook listener on http://{}.",
                host.listener.local_addr()
            ));
        }
        host.service = service;
        return Ok(Some(host));
    }

    let listener = WebhookListener::bind(
        listener_config,
        service.clone(),
        Arc::clone(&authenticator),
        options.webhook_allow_browser_origins.clone(),
    )
    .map_err(|error| anyhow!("failed to bind webhook listener on {address}: {error}"))?;
    let listening_address = listener.local_addr();
    console::info(format_args!(
        "Serving {} webhook trigger{} on http://{}.",
        service.len(),
        if service.len() == 1 { "" } else { "s" },
        listening_address
    ));
    Ok(Some(WebhookHost {
        executor: TriggerExecutor::new(
            core,
            store,
            "webhook",
            cancellation,
            options.trigger_monitor.clone(),
        )
        .map_err(|error| anyhow!("failed to start webhook executor: {error}"))?,
        listener,
        pending: BTreeMap::new(),
        service,
    }))
}

fn overloaded_response() -> WebhookResponse {
    text_response(503, "Webhook executor is at capacity. Try again later.")
}

fn unavailable_response() -> WebhookResponse {
    text_response(503, "Webhook service is stopping or reloading.")
}

fn dispatch_failed_response(error: &str) -> WebhookResponse {
    text_response(500, format!("Webhook dispatch failed: {error}"))
}

/// Names what the activation did, so a caller can tell a run from a toggle
/// without parsing the body. The configured response status is the author's
/// to choose, so the status code alone cannot carry this.
fn with_outcome_header(mut response: WebhookResponse, outcome: &str) -> WebhookResponse {
    response
        .headers
        .insert("X-BaudBound-Trigger-Outcome".to_owned(), outcome.to_owned());
    response
}

fn text_response(status_code: u16, body: impl Into<String>) -> WebhookResponse {
    WebhookResponse {
        body: body.into(),
        content_type: "text/plain".to_owned(),
        headers: BTreeMap::new(),
        status_code,
    }
}

#[cfg(test)]
#[path = "webhooks/tests.rs"]
mod tests;
