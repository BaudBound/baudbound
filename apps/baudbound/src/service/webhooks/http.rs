use std::{collections::BTreeMap, io::Read};

use baudbound_triggers::{WebhookRequest, WebhookResponse};
use tiny_http::{Header, Request, Response, StatusCode};

pub(super) fn request_from_http(
    request: &mut Request,
    max_body_bytes: usize,
) -> Result<WebhookRequest, WebhookResponse> {
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
