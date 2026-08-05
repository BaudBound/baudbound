use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    time::{Duration, Instant},
};

use baudbound_runtime::{
    ResourceLimit, RuntimeActionError, RuntimeActionFailure, RuntimeActionRequest,
    RuntimeActionResult, RuntimeContext,
};
use baudbound_security::{all_network_addresses_are_public, is_https_downgrade, same_http_origin};
use reqwest::{
    Method, StatusCode,
    blocking::{Client, Response},
    header::{CONTENT_LENGTH, HeaderMap, HeaderName, HeaderValue, TRANSFER_ENCODING},
    redirect::Policy,
};
use serde_json::{Map, Number, Value};
use url::{Host, Url};

use crate::{
    ActionSecurityPolicy, actions::bounded_io, config_string, number_from_config, required_string,
    timeout_duration, value_kind, value_to_string,
};

pub(crate) fn http_request_action(
    request: &RuntimeActionRequest,
    max_response_bytes: ResourceLimit,
    policy: &ActionSecurityPolicy,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let method = request_method(request)?;
    let url = required_string(request, "url")
        .map_err(|error| reclassify_http_error(request, "INVALID_URL", false, error))?;
    let mut url = Url::parse(&url).map_err(|source| {
        http_error(
            request,
            "INVALID_URL",
            format!("invalid HTTP URL: {source}"),
            false,
        )
    })?;
    let timeout = timeout_duration(request)
        .map_err(|error| reclassify_http_error(request, "INVALID_TIMEOUT", false, error))?;
    let mut headers = request_headers(request)?;
    let mut user_agent = config_string(&request.config, "userAgent");
    let body = config_string(&request.config, "body").unwrap_or_default();
    validate_json_request_body(request, &headers, &body)?;

    let started_at = Instant::now();
    let mut method = method;
    let mut body = body;
    let mut redirects = 0_u8;
    let mut visited = HashSet::from([url.as_str().to_owned()]);
    let mut response = loop {
        let client = validated_http_client(request, &url, timeout, policy)?;
        let safe_url = safe_http_destination(url.as_str());
        let mut builder = client
            .request(method.clone(), url.clone())
            .headers(headers.clone());
        if let Some(user_agent) = user_agent
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            builder = builder.header(reqwest::header::USER_AGENT, user_agent);
        }
        if method_allows_body(&method) && !body.is_empty() {
            builder = builder.body(body.clone());
        }
        let response = builder.send().map_err(|source| {
            let code = if source.is_timeout() {
                "HTTP_TIMEOUT"
            } else {
                "HTTP_REQUEST_FAILED"
            };
            http_error(
                request,
                code,
                format!(
                    "HTTP request {method} {safe_url} failed: {}",
                    sanitized_http_error(&source, url.as_str(), &safe_url)
                ),
                true,
            )
        })?;
        let Some(next_url) = redirect_target(request, &url, &response)? else {
            break response;
        };
        if redirects >= 10 {
            return http_failed(
                request,
                "REDIRECT_LIMIT",
                "HTTP request exceeded 10 redirects",
                false,
            );
        }
        validate_redirect_transition(request, &url, &next_url)?;
        let switch_to_get = redirects_switch_to_get(response.status(), &method);
        if !same_http_origin(&url, &next_url) {
            headers.clear();
            user_agent = None;
            if !switch_to_get && method_allows_body(&method) && !body.is_empty() {
                return http_failed(
                    request,
                    "CROSS_ORIGIN_BODY_BLOCKED",
                    "HTTP request refused to forward a request body across origins during redirect",
                    false,
                );
            }
        }
        if switch_to_get {
            method = Method::GET;
            body.clear();
            headers.remove(CONTENT_LENGTH);
            headers.remove(TRANSFER_ENCODING);
        }
        if !visited.insert(next_url.as_str().to_owned()) {
            return http_failed(
                request,
                "REDIRECT_LOOP",
                "HTTP request encountered a redirect loop",
                false,
            );
        }
        redirects += 1;
        url = next_url;
    };
    let duration_ms = elapsed_millis(started_at);
    let status = response.status();
    let headers = response_headers(response.headers());
    if response
        .content_length()
        .is_some_and(|length| max_response_bytes.is_exceeded_by(length))
    {
        return http_failed(
            request,
            "RESPONSE_BODY_LIMIT",
            format!(
                "HTTP response body exceeds the configured limit of {max_response_bytes} bytes"
            ),
            false,
        );
    }
    let body = bounded_io::read_to_end(&mut response, max_response_bytes).map_err(|source| {
        let (code, retryable) = match source {
            bounded_io::BoundedIoError::LimitExceeded { .. } => ("RESPONSE_BODY_LIMIT", false),
            bounded_io::BoundedIoError::Io(_) => ("HTTP_REQUEST_FAILED", true),
        };
        http_error(
            request,
            code,
            format!("failed to read HTTP response body: {source}"),
            retryable,
        )
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

    Ok(RuntimeActionResult::new(output_data))
}

pub(crate) fn send_download_request(
    request: &RuntimeActionRequest,
    url: &str,
    timeout: Duration,
    policy: &ActionSecurityPolicy,
) -> Result<Response, RuntimeActionError> {
    let mut url = Url::parse(url).map_err(|source| {
        http_error(
            request,
            "INVALID_URL",
            format!("invalid download URL: {source}"),
            false,
        )
    })?;
    let mut visited = HashSet::from([url.as_str().to_owned()]);
    for redirects in 0_u8..=10 {
        let client = validated_http_client(request, &url, timeout, policy)?;
        let safe_url = safe_http_destination(url.as_str());
        let response = client.get(url.clone()).send().map_err(|source| {
            let code = if source.is_timeout() {
                "HTTP_TIMEOUT"
            } else {
                "HTTP_REQUEST_FAILED"
            };
            http_error(
                request,
                code,
                format!(
                    "download request {safe_url} failed: {}",
                    sanitized_http_error(&source, url.as_str(), &safe_url)
                ),
                true,
            )
        })?;
        let Some(next_url) = redirect_target(request, &url, &response)? else {
            return Ok(response);
        };
        if redirects == 10 {
            return http_failed(
                request,
                "REDIRECT_LIMIT",
                "download request exceeded 10 redirects",
                false,
            );
        }
        validate_redirect_transition(request, &url, &next_url)?;
        if !visited.insert(next_url.as_str().to_owned()) {
            return http_failed(
                request,
                "REDIRECT_LOOP",
                "download request encountered a redirect loop",
                false,
            );
        }
        url = next_url;
    }
    unreachable!("redirect loop always returns or advances to its fixed limit")
}

fn validated_http_client(
    request: &RuntimeActionRequest,
    url: &Url,
    timeout: Duration,
    policy: &ActionSecurityPolicy,
) -> Result<Client, RuntimeActionError> {
    validate_http_url(request, url)?;
    let mut builder = Client::builder()
        .timeout(timeout)
        .redirect(Policy::none())
        .no_proxy();
    if !policy.allow_private_http_requests {
        let addresses = resolved_destination_addresses(request, url)?;
        if !all_network_addresses_are_public(addresses.iter().copied()) {
            return http_failed(
                request,
                "PRIVATE_ADDRESS_BLOCKED",
                format!(
                    "HTTP request to private or local network destination {} is blocked because security.policy.allow_private_http_requests is false",
                    safe_http_destination(url.as_str())
                ),
                false,
            );
        }
        if let Some(Host::Domain(host)) = url.host() {
            let port = url
                .port_or_known_default()
                .expect("validated HTTP schemes always have a known default port");
            let socket_addresses = addresses
                .into_iter()
                .map(|address| SocketAddr::new(address, port))
                .collect::<Vec<_>>();
            builder = builder.resolve_to_addrs(host, &socket_addresses);
        }
    }
    builder.build().map_err(|source| {
        http_error(
            request,
            "HTTP_REQUEST_FAILED",
            format!("failed to build HTTP client: {source}"),
            true,
        )
    })
}

fn resolved_destination_addresses(
    request: &RuntimeActionRequest,
    url: &Url,
) -> Result<Vec<IpAddr>, RuntimeActionError> {
    let port = url.port_or_known_default().ok_or_else(|| {
        http_error(
            request,
            "INVALID_URL",
            "HTTP URL is missing a port and has no known default",
            false,
        )
    })?;
    match url.host() {
        Some(Host::Ipv4(address)) => Ok(vec![IpAddr::V4(address)]),
        Some(Host::Ipv6(address)) => Ok(vec![IpAddr::V6(address)]),
        Some(Host::Domain(host)) => {
            let addresses = (host, port)
                .to_socket_addrs()
                .map_err(|source| {
                    http_error(
                        request,
                        "DNS_FAILED",
                        format!("failed to resolve HTTP host {host}: {source}"),
                        true,
                    )
                })?
                .map(|address| address.ip())
                .collect::<Vec<_>>();
            if addresses.is_empty() {
                return http_failed(
                    request,
                    "DNS_FAILED",
                    format!("HTTP host {host} did not resolve"),
                    true,
                );
            }
            Ok(addresses)
        }
        None => http_failed(request, "INVALID_URL", "HTTP URL is missing a host", false),
    }
}

fn redirect_target(
    request: &RuntimeActionRequest,
    current_url: &Url,
    response: &reqwest::blocking::Response,
) -> Result<Option<Url>, RuntimeActionError> {
    if !response.status().is_redirection() {
        return Ok(None);
    }
    let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
        return Ok(None);
    };
    let location = location.to_str().map_err(|source| {
        http_error(
            request,
            "INVALID_HEADERS",
            format!("HTTP redirect location is not valid header text: {source}"),
            false,
        )
    })?;
    current_url.join(location).map(Some).map_err(|source| {
        http_error(
            request,
            "INVALID_URL",
            format!("HTTP redirect location is not a valid URL: {source}"),
            false,
        )
    })
}

fn validate_http_url(request: &RuntimeActionRequest, url: &Url) -> Result<(), RuntimeActionError> {
    if !matches!(url.scheme(), "http" | "https") {
        return http_failed(
            request,
            "INVALID_SCHEME",
            "HTTP URL scheme must be http or https",
            false,
        );
    }
    if url.host().is_none() {
        return http_failed(request, "INVALID_URL", "HTTP URL is missing a host", false);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return http_failed(
            request,
            "URL_CREDENTIALS_BLOCKED",
            "HTTP URL credentials are not allowed; use an explicit authorization header",
            false,
        );
    }
    Ok(())
}

fn validate_redirect_transition(
    request: &RuntimeActionRequest,
    current_url: &Url,
    next_url: &Url,
) -> Result<(), RuntimeActionError> {
    validate_http_url(request, next_url)?;
    if is_https_downgrade(current_url, next_url) {
        return http_failed(
            request,
            "REDIRECT_DOWNGRADE",
            "HTTP redirect from HTTPS to HTTP is not allowed",
            false,
        );
    }
    Ok(())
}

fn redirects_switch_to_get(status: StatusCode, method: &Method) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND | StatusCode::SEE_OTHER
    ) && *method != Method::GET
        && *method != Method::HEAD
}

fn safe_http_destination(value: &str) -> String {
    let Ok(url) = url::Url::parse(value) else {
        return "[INVALID URL]".to_owned();
    };
    let mut destination = format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default());
    if let Some(port) = url.port() {
        destination.push(':');
        destination.push_str(&port.to_string());
    }
    destination.push_str(&redacted_path(url.path()));
    let query_names = url
        .query_pairs()
        .map(|(name, _)| format!("{name}=[REDACTED]"))
        .collect::<Vec<_>>();
    if !query_names.is_empty() {
        destination.push('?');
        destination.push_str(&query_names.join("&"));
    }
    destination
}

/// Redacts path segments that carry enough entropy to be a credential.
///
/// Query values were already redacted, but the path was emitted verbatim, so a
/// key embedded in a REST path or a signed URL segment reached the run log. A
/// short segment is kept because it is almost always a route name that the
/// author needs in order to recognise the request.
fn redacted_path(path: &str) -> String {
    const MAX_PLAIN_SEGMENT_CHARS: usize = 24;

    path.split('/')
        .map(|segment| {
            if segment.chars().count() > MAX_PLAIN_SEGMENT_CHARS {
                "[REDACTED]"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn sanitized_http_error(error: &reqwest::Error, original_url: &str, safe_url: &str) -> String {
    let mut message = error.to_string().replace(original_url, safe_url);
    if let Ok(url) = url::Url::parse(original_url) {
        for (_, value) in url.query_pairs() {
            if !value.is_empty() {
                message = message.replace(value.as_ref(), "[REDACTED]");
            }
        }
        if !url.username().is_empty() {
            message = message.replace(url.username(), "[REDACTED]");
        }
        if let Some(password) = url.password().filter(|password| !password.is_empty()) {
            message = message.replace(password, "[REDACTED]");
        }
    }
    message
}

fn http_error(
    request: &RuntimeActionRequest,
    code: &'static str,
    message: impl Into<String>,
    retryable: bool,
) -> RuntimeActionError {
    let mut failure = RuntimeActionFailure::new(code, "http", message, retryable);
    if let Some(method) = config_string(&request.config, "method") {
        failure = failure.with_detail("method", Value::String(method));
    }
    if let Some(url) = config_string(&request.config, "url") {
        failure = failure.with_detail(
            "destination",
            Value::String(safe_http_origin(&url).unwrap_or_else(|| "[INVALID URL]".to_owned())),
        );
    }
    RuntimeActionError::StructuredFailure {
        action_type: request.action_type.clone(),
        failure,
    }
}

fn http_failed<T>(
    request: &RuntimeActionRequest,
    code: &'static str,
    message: impl Into<String>,
    retryable: bool,
) -> Result<T, RuntimeActionError> {
    Err(http_error(request, code, message, retryable))
}

fn reclassify_http_error(
    request: &RuntimeActionRequest,
    code: &'static str,
    retryable: bool,
    error: RuntimeActionError,
) -> RuntimeActionError {
    match error {
        RuntimeActionError::Failed { message, .. } => http_error(request, code, message, retryable),
        other => other,
    }
}

fn safe_http_origin(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    let host = url.host_str()?;
    let mut origin = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        origin.push(':');
        origin.push_str(&port.to_string());
    }
    Some(origin)
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
        sensitive_output_keys: Default::default(),
    })
}

fn http_status_config(
    request: &RuntimeActionRequest,
    key: &str,
    fallback: u16,
) -> Result<u16, RuntimeActionError> {
    let status = number_from_config(&request.config, key).unwrap_or(f64::from(fallback));
    if !status.is_finite() || status.fract() != 0.0 || !(100.0..=599.0).contains(&status) {
        return http_failed(
            request,
            "INVALID_STATUS",
            format!("{key} must be an HTTP status code 100-599"),
            false,
        );
    }
    Ok(status as u16)
}

fn request_method(request: &RuntimeActionRequest) -> Result<Method, RuntimeActionError> {
    let method = config_string(&request.config, "method").unwrap_or_else(|| "GET".to_owned());
    Method::from_bytes(method.trim().as_bytes()).map_err(|source| {
        http_error(
            request,
            "INVALID_METHOD",
            format!("invalid HTTP method {method}: {source}"),
            false,
        )
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
            return http_failed(
                request,
                "INVALID_HEADERS",
                format!(
                    "headers must be a list or object, found {}",
                    value_kind(other)
                ),
                false,
            );
        }
    }
    Ok(headers)
}

fn validate_json_request_body(
    request: &RuntimeActionRequest,
    headers: &HeaderMap,
    body: &str,
) -> Result<(), RuntimeActionError> {
    if body.is_empty() || !has_json_content_type(headers) {
        return Ok(());
    }
    serde_json::from_str::<Value>(body)
        .map(|_| ())
        .map_err(|source| {
            http_error(
                request,
                "INVALID_REQUEST",
                format!(
                    "HTTP request body is not valid JSON for its Content-Type header: {source}"
                ),
                false,
            )
        })
}

fn has_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            let media_type = value.split(';').next().unwrap_or_default().trim();
            media_type.eq_ignore_ascii_case("application/json")
                || media_type
                    .to_ascii_lowercase()
                    .strip_prefix("application/")
                    .is_some_and(|subtype| subtype.ends_with("+json"))
        })
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
    let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|source| {
        http_error(
            request,
            "INVALID_HEADERS",
            format!("invalid HTTP header name {name}: {source}"),
            false,
        )
    })?;
    let header_value = HeaderValue::from_str(&value).map_err(|source| {
        http_error(
            request,
            "INVALID_HEADERS",
            format!("invalid HTTP header value for {name}: {source}"),
            false,
        )
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
