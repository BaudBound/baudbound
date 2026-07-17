use std::{collections::BTreeMap, io::Read};

use baudbound_triggers::{WebhookRequest, WebhookResponse};
use tiny_http::{Header, Request, Response, StatusCode};

const TOKEN_HEADER: &str = "x-baudbound-token";

pub(super) struct ParsedWebhookRequest {
    pub(super) origin: Option<String>,
    pub(super) request: WebhookRequest,
    pub(super) token: Option<String>,
}

pub(super) fn request_from_http(
    request: &mut Request,
    max_body_bytes: usize,
) -> Result<ParsedWebhookRequest, WebhookResponse> {
    let mut body = Vec::new();
    request
        .as_reader()
        .take(max_body_bytes.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .map_err(|source| text_response(400, format!("Failed to read request body: {source}")))?;
    if body.len() > max_body_bytes {
        return Err(text_response(
            413,
            format!("Request body exceeds {max_body_bytes} bytes."),
        ));
    }

    let token = header_value(request, TOKEN_HEADER);
    let origin = header_value(request, "origin");
    let request = WebhookRequest {
        body: String::from_utf8_lossy(&body).into_owned(),
        headers: request
            .headers()
            .iter()
            .filter(|header| !header.field.equiv(TOKEN_HEADER))
            .map(|header| {
                (
                    header.field.as_str().to_ascii_lowercase().to_string(),
                    header.value.as_str().to_owned(),
                )
            })
            .collect(),
        method: request.method().to_string(),
        path_and_query: request.url().to_owned(),
    };
    Ok(ParsedWebhookRequest {
        origin,
        request,
        token,
    })
}

pub(super) fn preflight_response(
    request: &Request,
    allowed_origins: &std::collections::BTreeSet<String>,
) -> Option<WebhookResponse> {
    if !request.method().as_str().eq_ignore_ascii_case("OPTIONS") {
        return None;
    }
    let origin = header_value(request, "origin")?;
    let requested_method = header_value(request, "access-control-request-method")?;
    if !allowed_origins.contains(&origin) {
        return Some(text_response(403, "Browser origin is not allowed."));
    }
    let requested_headers =
        header_value(request, "access-control-request-headers").unwrap_or_default();
    if requested_headers.split(',').map(str::trim).any(|header| {
        !header.is_empty()
            && !header.eq_ignore_ascii_case("content-type")
            && !header.eq_ignore_ascii_case(TOKEN_HEADER)
    }) {
        return Some(text_response(
            403,
            "Requested browser headers are not allowed.",
        ));
    }

    let mut response = WebhookResponse {
        body: String::new(),
        content_type: "text/plain".to_owned(),
        headers: BTreeMap::new(),
        status_code: 204,
    };
    response
        .headers
        .insert("Access-Control-Allow-Origin".to_owned(), origin);
    response.headers.insert(
        "Access-Control-Allow-Methods".to_owned(),
        requested_method.to_ascii_uppercase(),
    );
    response.headers.insert(
        "Access-Control-Allow-Headers".to_owned(),
        "Content-Type, X-BaudBound-Token".to_owned(),
    );
    response
        .headers
        .insert("Vary".to_owned(), "Origin".to_owned());
    Some(response)
}

pub(super) fn with_cors_origin(
    mut response: WebhookResponse,
    origin: Option<&str>,
) -> WebhookResponse {
    if let Some(origin) = origin {
        response
            .headers
            .insert("Access-Control-Allow-Origin".to_owned(), origin.to_owned());
        response
            .headers
            .insert("Vary".to_owned(), "Origin".to_owned());
    }
    response
}

fn header_value(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(super) fn respond_safely(request: Request, webhook_response: WebhookResponse) {
    if let Err(error) = respond(request, webhook_response) {
        tracing::warn!("failed to write webhook response: {error}");
    }
}

fn respond(request: Request, webhook_response: WebhookResponse) -> std::io::Result<()> {
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
    request.respond(response)
}

fn text_response(status_code: u16, body: impl Into<String>) -> WebhookResponse {
    WebhookResponse {
        body: body.into(),
        content_type: "text/plain".to_owned(),
        headers: BTreeMap::new(),
        status_code,
    }
}
