use std::collections::{BTreeMap, BTreeSet};

use baudbound_triggers::{WebhookRequest, WebhookResponse};

const AUTHORIZATION_HEADER: &str = "authorization";

pub(super) struct ParsedWebhookRequest {
    pub(super) origin: Option<String>,
    pub(super) request: WebhookRequest,
    pub(super) token: Option<String>,
}

pub(super) fn preflight_response(
    parsed: &ParsedWebhookRequest,
    allowed_origins: &BTreeSet<String>,
) -> Option<WebhookResponse> {
    if !parsed.request.method.eq_ignore_ascii_case("OPTIONS") {
        return None;
    }
    let origin = parsed.origin.as_ref()?;
    let requested_method = request_header(parsed, "access-control-request-method")?;
    if !allowed_origins.contains(origin.as_str()) {
        return Some(text_response(403, "Browser origin is not allowed."));
    }
    let requested_headers = request_header(parsed, "access-control-request-headers")
        .map_or_else(String::new, ToOwned::to_owned);
    if requested_headers.split(',').map(str::trim).any(|header| {
        !header.is_empty()
            && !header.eq_ignore_ascii_case("content-type")
            && !header.eq_ignore_ascii_case(AUTHORIZATION_HEADER)
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
        .insert("Access-Control-Allow-Origin".to_owned(), origin.to_owned());
    response.headers.insert(
        "Access-Control-Allow-Methods".to_owned(),
        requested_method.to_ascii_uppercase(),
    );
    response.headers.insert(
        "Access-Control-Allow-Headers".to_owned(),
        "Content-Type, Authorization".to_owned(),
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

fn request_header<'a>(request: &'a ParsedWebhookRequest, name: &str) -> Option<&'a str> {
    request
        .request
        .headers
        .iter()
        .find(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
}

fn text_response(status_code: u16, body: impl Into<String>) -> WebhookResponse {
    WebhookResponse {
        body: body.into(),
        content_type: "text/plain".to_owned(),
        headers: BTreeMap::new(),
        status_code,
    }
}
