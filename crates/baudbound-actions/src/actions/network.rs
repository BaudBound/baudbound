use std::time::Instant;

use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use reqwest::{
    Method, StatusCode,
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Map, Number, Value};

use crate::{
    actions::bounded_io, config_string, failed, number_from_config, required_string,
    timeout_duration, value_kind, value_to_string,
};

pub(crate) fn http_request_action(
    request: &RuntimeActionRequest,
    max_response_bytes: u64,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let method = request_method(request)?;
    let url = required_string(request, "url")?;
    let timeout = timeout_duration(request)?;
    let headers = request_headers(request)?;
    let user_agent = config_string(&request.config, "userAgent");
    let body = config_string(&request.config, "body").unwrap_or_default();

    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to build HTTP client: {source}"),
        })?;

    let started_at = Instant::now();
    let mut builder = client.request(method.clone(), &url).headers(headers);
    if let Some(user_agent) = user_agent.filter(|value| !value.trim().is_empty()) {
        builder = builder.header(reqwest::header::USER_AGENT, user_agent);
    }
    if method_allows_body(&method) && !body.is_empty() {
        builder = builder.body(body);
    }

    let mut response = builder
        .send()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("HTTP request {method} {url} failed: {source}"),
        })?;
    let duration_ms = elapsed_millis(started_at);
    let status = response.status();
    let headers = response_headers(response.headers());
    if response
        .content_length()
        .is_some_and(|length| length > max_response_bytes)
    {
        return failed(
            request,
            format!(
                "HTTP response body exceeds the configured limit of {max_response_bytes} bytes"
            ),
        );
    }
    let body = bounded_io::read_to_end(&mut response, max_response_bytes).map_err(|source| {
        RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to read HTTP response body: {source}"),
        }
    })?;
    let body = String::from_utf8_lossy(&body).into_owned();
    let json_body = serde_json::from_str::<Value>(&body).ok();

    let mut output_data = Map::from_iter([
        (
            "status_code".to_owned(),
            Value::Number(Number::from(status.as_u16())),
        ),
        (
            "status_text".to_owned(),
            Value::String(status_text(status).to_owned()),
        ),
        ("headers".to_owned(), Value::Object(headers)),
        ("body".to_owned(), Value::String(body)),
        (
            "duration_ms".to_owned(),
            Value::Number(Number::from(duration_ms)),
        ),
    ]);
    if let Some(json_body) = json_body {
        output_data.insert("json".to_owned(), json_body);
    }

    Ok(RuntimeActionResult { output_data })
}

pub(crate) fn webhook_response_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let status_code = http_status_config(request, "statusCode", 200)?;
    let content_type =
        config_string(&request.config, "contentType").unwrap_or_else(|| "text/plain".to_owned());
    let body = config_string(&request.config, "body").unwrap_or_default();
    let headers = request_headers(request)?;
    let trigger_id = context
        .trigger_payload
        .get("trigger_id")
        .and_then(Value::as_str)
        .unwrap_or(&context.identity.trigger_node_id)
        .to_owned();

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("sent".to_owned(), Value::Bool(true)),
            (
                "status_code".to_owned(),
                Value::Number(Number::from(status_code)),
            ),
            ("content_type".to_owned(), Value::String(content_type)),
            (
                "headers".to_owned(),
                Value::Object(response_headers(&headers)),
            ),
            ("body".to_owned(), Value::String(body)),
            ("trigger_id".to_owned(), Value::String(trigger_id)),
        ]),
    })
}

fn http_status_config(
    request: &RuntimeActionRequest,
    key: &str,
    fallback: u16,
) -> Result<u16, RuntimeActionError> {
    let status = number_from_config(&request.config, key).unwrap_or(f64::from(fallback));
    if !status.is_finite() || status.fract() != 0.0 || !(100.0..=599.0).contains(&status) {
        return failed(
            request,
            format!("{key} must be an HTTP status code 100-599"),
        );
    }
    Ok(status as u16)
}

fn request_method(request: &RuntimeActionRequest) -> Result<Method, RuntimeActionError> {
    let method = config_string(&request.config, "method").unwrap_or_else(|| "GET".to_owned());
    Method::from_bytes(method.trim().as_bytes()).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("invalid HTTP method {method}: {source}"),
    })
}

fn request_headers(request: &RuntimeActionRequest) -> Result<HeaderMap, RuntimeActionError> {
    let mut headers = HeaderMap::new();
    match request.config.get("headers") {
        Some(Value::Array(rows)) => {
            for row in rows {
                let Some(row) = row.as_object() else {
                    continue;
                };
                let name = row.get("name").map(value_to_string).unwrap_or_default();
                let value = row.get("value").map(value_to_string).unwrap_or_default();
                insert_header(request, &mut headers, name, value)?;
            }
        }
        Some(Value::Object(values)) => {
            for (name, value) in values {
                insert_header(request, &mut headers, name.clone(), value_to_string(value))?;
            }
        }
        Some(Value::Null) | None => {}
        Some(other) => {
            return failed(
                request,
                format!(
                    "headers must be a list or object, found {}",
                    value_kind(other)
                ),
            );
        }
    }
    Ok(headers)
}

fn insert_header(
    request: &RuntimeActionRequest,
    headers: &mut HeaderMap,
    name: String,
    value: String,
) -> Result<(), RuntimeActionError> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(());
    }
    let header_name =
        HeaderName::from_bytes(name.as_bytes()).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("invalid HTTP header name {name}: {source}"),
        })?;
    let header_value =
        HeaderValue::from_str(&value).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("invalid HTTP header value for {name}: {source}"),
        })?;
    headers.insert(header_name, header_value);
    Ok(())
}

fn response_headers(headers: &HeaderMap) -> Map<String, Value> {
    let mut values = Map::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            values.insert(name.as_str().to_owned(), Value::String(value.to_owned()));
        }
    }
    values
}

fn status_text(status: StatusCode) -> &'static str {
    status.canonical_reason().unwrap_or("")
}

fn method_allows_body(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD)
}

fn elapsed_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}
