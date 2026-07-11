use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use baudbound_core::{RunnerCore, TriggerRegistration};
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::{WebhookDispatch, WebhookResponse, WebhookService};
use tiny_http::{Request, Server};

use super::{
    executor::{TriggerCompletion, TriggerExecutor, TriggerSubmitError},
    heartbeat::ServeStatusTracker,
    options::ServeOptions,
};

mod http;

use http::{request_from_http, respond_safely};

const COMPLETION_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) struct WebhookHost {
    executor: TriggerExecutor,
    pending: BTreeMap<u64, PendingWebhookResponse>,
    pub(super) server: Server,
    pub(super) service: WebhookService,
}

struct PendingWebhookResponse {
    deadline: Instant,
    dispatch: WebhookDispatch,
    request: Request,
}

impl WebhookHost {
    pub(super) fn accept_request(&mut self, mut request: Request, max_body_bytes: usize) {
        let webhook_request = match request_from_http(&mut request, max_body_bytes) {
            Ok(request) => request,
            Err(response) => {
                respond_safely(request, response);
                return;
            }
        };

        let Some(dispatch) = self.service.dispatch_for_request(&webhook_request) else {
            respond_safely(request, route_not_found_response());
            return;
        };

        println!(
            "Queueing webhook trigger {} for script {}",
            dispatch.event.node_id, dispatch.event.script_id
        );
        let job_id = match self.executor.submit(dispatch.event.clone()) {
            Ok(job_id) => job_id,
            Err(TriggerSubmitError::Full) => {
                respond_safely(request, overloaded_response());
                return;
            }
            Err(TriggerSubmitError::Stopped) => {
                respond_safely(request, unavailable_response());
                return;
            }
        };

        if dispatch.wait_for_response {
            self.pending.insert(
                job_id,
                PendingWebhookResponse {
                    deadline: Instant::now() + dispatch.response_timeout,
                    dispatch,
                    request,
                },
            );
        } else {
            respond_safely(request, dispatch.fallback_response);
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
            Ok(report) => {
                status.record_report("webhook", &report);
                if let Some(pending) = self.pending.remove(&completion.job_id) {
                    let response = self.service.response_for_report(&pending.dispatch, &report);
                    respond_safely(pending.request, response);
                }
            }
            Err(error) => {
                status.record_event_failure("webhook", &completion.event, error.clone());
                if let Some(pending) = self.pending.remove(&completion.job_id) {
                    respond_safely(pending.request, dispatch_failed_response(&error));
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
                respond_safely(pending.request, pending.dispatch.fallback_response);
            }
        }
    }
}

impl Drop for WebhookHost {
    fn drop(&mut self) {
        for (_, pending) in std::mem::take(&mut self.pending) {
            respond_safely(pending.request, unavailable_response());
        }
    }
}

pub(super) fn build_webhook_host(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    registrations: Vec<TriggerRegistration>,
    options: &ServeOptions,
    previous_webhook_host: Option<WebhookHost>,
) -> Result<Option<WebhookHost>> {
    if !options.webhooks_enabled {
        return Ok(None);
    }

    let service = WebhookService::from_registrations(registrations)
        .context("failed to register webhook triggers")?;
    if service.is_empty() {
        println!("No enabled webhook triggers found.");
        return Ok(None);
    }

    if let Some(mut host) = previous_webhook_host {
        host.service = service;
        return Ok(Some(host));
    }

    let address = format!("{}:{}", options.webhook_bind, options.webhook_port);
    let server = Server::http(&address)
        .map_err(|error| anyhow!("failed to bind webhook listener on {address}: {error}"))?;
    println!(
        "Serving {} webhook trigger{} on http://{}.",
        service.len(),
        if service.len() == 1 { "" } else { "s" },
        address
    );
    Ok(Some(WebhookHost {
        executor: TriggerExecutor::new(core, store, "webhook")
            .map_err(|error| anyhow!("failed to start webhook executor: {error}"))?,
        pending: BTreeMap::new(),
        server,
        service,
    }))
}

fn route_not_found_response() -> WebhookResponse {
    text_response(404, "Webhook route not found.")
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
