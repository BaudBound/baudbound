use std::{collections::BTreeMap, io::Read};

use anyhow::{Context, Result, anyhow};
use baudbound_core::{RunnerCore, TriggerRegistration};
use baudbound_storage::FilesystemScriptStore;
use baudbound_triggers::{WebhookRequest, WebhookResponse, WebhookService};
use tiny_http::{Header, Request, Response, Server, StatusCode};

use super::{heartbeat::ServeStatusTracker, options::ServeOptions};

pub(super) struct WebhookHost {
    pub(super) server: Server,
    pub(super) service: WebhookService,
}

pub(super) fn build_webhook_host(
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
    Ok(Some(WebhookHost { server, service }))
}

pub(super) fn handle_webhook_request(
    core: &RunnerCore,
    store: &FilesystemScriptStore,
    service: &WebhookService,
    mut request: Request,
    max_body_bytes: usize,
    status: &mut ServeStatusTracker,
) -> Result<()> {
    let webhook_request = match webhook_request_from_http(&mut request, max_body_bytes) {
        Ok(request) => request,
        Err(response) => {
            respond(request, response)?;
            return Ok(());
        }
    };

    let Some(dispatch) = service.dispatch_for_request(&webhook_request) else {
        respond(
            request,
            WebhookResponse {
                body: "Webhook route not found.".to_owned(),
                content_type: "text/plain".to_owned(),
                headers: BTreeMap::new(),
                status_code: 404,
            },
        )?;
        return Ok(());
    };

    println!(
        "Dispatching webhook trigger {} for script {}",
        dispatch.event.node_id, dispatch.event.script_id
    );
    let response = match core.dispatch_trigger_event(store, dispatch.event.clone()) {
        Ok(report) => {
            status.record_report("webhook", &report);
            service.response_for_report(&dispatch, &report)
        }
        Err(error) => {
            status.record_event_failure("webhook", &dispatch.event, error.to_string());
            WebhookResponse {
                body: format!("Webhook dispatch failed: {error}"),
                content_type: "text/plain".to_owned(),
                headers: BTreeMap::new(),
                status_code: 500,
            }
        }
    };
    respond(request, response)?;
    Ok(())
}

fn webhook_request_from_http(
    request: &mut Request,
    max_body_bytes: usize,
) -> std::result::Result<WebhookRequest, WebhookResponse> {
    let mut body = Vec::new();
    request
        .as_reader()
        .take(max_body_bytes.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .map_err(|source| WebhookResponse {
            body: format!("Failed to read request body: {source}"),
            content_type: "text/plain".to_owned(),
            headers: BTreeMap::new(),
            status_code: 400,
        })?;
    if body.len() > max_body_bytes {
        return Err(WebhookResponse {
            body: format!("Request body exceeds {max_body_bytes} bytes."),
            content_type: "text/plain".to_owned(),
            headers: BTreeMap::new(),
            status_code: 413,
        });
    }

    Ok(WebhookRequest {
        body: String::from_utf8_lossy(&body).into_owned(),
        headers: request
            .headers()
            .iter()
            .map(|header| {
                (
                    header.field.as_str().to_ascii_lowercase().to_string(),
                    header.value.as_str().to_owned(),
                )
            })
            .collect(),
        method: request.method().to_string(),
        path_and_query: request.url().to_owned(),
    })
}

fn respond(request: Request, webhook_response: WebhookResponse) -> Result<()> {
    let mut response = Response::from_string(webhook_response.body)
        .with_status_code(StatusCode(webhook_response.status_code));
    if let Ok(header) = Header::from_bytes("Content-Type", webhook_response.content_type) {
        response.add_header(header);
    }
    for (name, value) in webhook_response.headers {
        if let Ok(header) = Header::from_bytes(name, value) {
            response.add_header(header);
        }
    }
    request
        .respond(response)
        .map_err(|error| anyhow!("failed to write HTTP response: {error}"))
}
