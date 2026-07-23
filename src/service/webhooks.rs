use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use baudbound_core::{RunnerCore, TriggerRegistration};
use baudbound_runtime::RuntimeCancellationToken;
use baudbound_storage::SqliteRunnerStore;
use baudbound_triggers::{
    NetworkTriggerAuthenticationError, NetworkTriggerAuthenticator, NetworkTriggerKind,
    WebhookDispatch, WebhookResponse, WebhookService,
};
use tiny_http::{Request, Server};

use crate::console;

use super::{
    executor::{TriggerCompletion, TriggerExecutor, TriggerSubmitError},
    heartbeat::ServeStatusTracker,
    network_auth::{RunnerNetworkTriggerAuthenticator, validate_listener_exposure},
    options::ServeOptions,
};

mod http;

use http::{preflight_response, request_from_http, respond_safely, with_cors_origin};

const COMPLETION_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) struct WebhookHost {
    allow_browser_origins: BTreeSet<String>,
    authenticator: Arc<dyn NetworkTriggerAuthenticator>,
    executor: TriggerExecutor,
    pending: BTreeMap<u64, PendingWebhookResponse>,
    pub(super) server: Server,
    pub(super) service: WebhookService,
}

struct PendingWebhookResponse {
    cors_origin: Option<String>,
    deadline: Instant,
    dispatch: WebhookDispatch,
    request: Request,
}

impl WebhookHost {
    pub(super) fn has_pending_execution(&self) -> bool {
        self.executor.has_pending()
    }

    pub(super) fn accept_request(&mut self, mut request: Request, max_body_bytes: usize) {
        if let Some(response) = preflight_response(&request, &self.allow_browser_origins) {
            respond_safely(request, response);
            return;
        }

        let parsed = match request_from_http(&mut request, max_body_bytes) {
            Ok(request) => request,
            Err(response) => {
                respond_safely(request, response);
                return;
            }
        };

        if let Some(origin) = parsed.origin.as_deref()
            && !self.allow_browser_origins.contains(origin)
        {
            respond_safely(request, browser_origin_denied_response());
            return;
        }

        let Some(dispatch) = self.service.dispatch_for_request(&parsed.request) else {
            respond_safely(request, route_not_found_response());
            return;
        };

        if let Err(error) = self.authenticator.authenticate(
            &dispatch.event.script_id,
            &dispatch.event.node_id,
            NetworkTriggerKind::Webhook,
            parsed.token.as_deref(),
        ) {
            respond_safely(
                request,
                with_cors_origin(
                    authentication_error_response(error),
                    parsed.origin.as_deref(),
                ),
            );
            return;
        }

        console::info(format_args!(
            "Queueing webhook trigger {} for script {}",
            dispatch.event.node_id, dispatch.event.script_id
        ));
        let job_id = match self.executor.submit_from(dispatch.event.clone(), "webhook") {
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
                    cors_origin: parsed.origin,
                    deadline: Instant::now() + dispatch.response_timeout,
                    dispatch,
                    request,
                },
            );
        } else {
            respond_safely(
                request,
                with_cors_origin(dispatch.fallback_response, parsed.origin.as_deref()),
            );
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
                    respond_safely(
                        pending.request,
                        with_cors_origin(response, pending.cors_origin.as_deref()),
                    );
                }
            }
            Err(error) => {
                status.record_event_failure("webhook", &completion.event, error.clone());
                if let Some(pending) = self.pending.remove(&completion.job_id) {
                    respond_safely(
                        pending.request,
                        with_cors_origin(
                            dispatch_failed_response(&error),
                            pending.cors_origin.as_deref(),
                        ),
                    );
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
                respond_safely(
                    pending.request,
                    with_cors_origin(
                        pending.dispatch.fallback_response,
                        pending.cors_origin.as_deref(),
                    ),
                );
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
        options.webhook_allow_unauthenticated_public_bind,
    )?;

    let service = WebhookService::from_registrations(registrations)
        .context("failed to register webhook triggers")?;
    if service.is_empty() {
        console::info(format_args!("No enabled webhook triggers found."));
        return Ok(None);
    }

    if let Some(mut host) = previous_webhook_host {
        host.allow_browser_origins = options.webhook_allow_browser_origins.clone();
        host.service = service;
        return Ok(Some(host));
    }

    let address = format!("{}:{}", options.webhook_bind, options.webhook_port);
    let server = Server::http(&address)
        .map_err(|error| anyhow!("failed to bind webhook listener on {address}: {error}"))?;
    console::info(format_args!(
        "Serving {} webhook trigger{} on http://{}.",
        service.len(),
        if service.len() == 1 { "" } else { "s" },
        address
    ));
    Ok(Some(WebhookHost {
        allow_browser_origins: options.webhook_allow_browser_origins.clone(),
        authenticator: Arc::new(RunnerNetworkTriggerAuthenticator::new(core, store)),
        executor: TriggerExecutor::new(
            core,
            store,
            "webhook",
            cancellation.clone(),
            options.trigger_monitor.clone(),
        )
        .map_err(|error| anyhow!("failed to start webhook executor: {error}"))?,
        pending: BTreeMap::new(),
        server,
        service,
    }))
}

fn route_not_found_response() -> WebhookResponse {
    text_response(404, "Webhook route not found.")
}

fn browser_origin_denied_response() -> WebhookResponse {
    text_response(403, "Browser origin is not allowed.")
}

fn authentication_error_response(error: NetworkTriggerAuthenticationError) -> WebhookResponse {
    match error {
        NetworkTriggerAuthenticationError::MissingToken => {
            text_response(401, "Webhook token is required.")
        }
        NetworkTriggerAuthenticationError::InvalidToken => {
            text_response(403, "Webhook token is invalid.")
        }
        NetworkTriggerAuthenticationError::Unavailable(error) => {
            tracing::error!("webhook authentication state is unavailable: {error}");
            text_response(503, "Webhook authentication is unavailable.")
        }
    }
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
