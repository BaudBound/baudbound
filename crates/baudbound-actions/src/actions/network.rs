use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    time::{Duration, Instant},
};

use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use reqwest::{
    Method, StatusCode,
    blocking::{Client, Response},
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};
use serde_json::{Map, Number, Value};
use url::{Host, Url};

use crate::{
    ActionSecurityPolicy, actions::bounded_io, config_string, failed, number_from_config,
    required_string, timeout_duration, value_kind, value_to_string,
};

pub(crate) fn http_request_action(
    request: &RuntimeActionRequest,
    max_response_bytes: u64,
    policy: &ActionSecurityPolicy,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let method = request_method(request)?;
    let url = required_string(request, "url")?;
    let mut url = Url::parse(&url).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("invalid HTTP URL: {source}"),
    })?;
    let timeout = timeout_duration(request)?;
    let headers = request_headers(request)?;
    let user_agent = config_string(&request.config, "userAgent");
    let body = config_string(&request.config, "body").unwrap_or_default();
    validate_json_request_body(request, &headers, &body)?;

    let started_at = Instant::now();
    let mut method = method;
    let mut body = body;
    let mut redirects = 0_u8;
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
        let response = builder
            .send()
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!(
                    "HTTP request {method} {safe_url} failed: {}",
                    sanitized_http_error(&source, url.as_str(), &safe_url)
                ),
            })?;
        let Some(next_url) = redirect_target(request, &url, &response)? else {
            break response;
        };
        if redirects >= 10 {
            return failed(request, "HTTP request exceeded 10 redirects");
        }
        if redirects_switch_to_get(response.status(), &method) {
            method = Method::GET;
            body.clear();
        }
        redirects += 1;
        url = next_url;
    };
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

pub(crate) fn send_download_request(
    request: &RuntimeActionRequest,
    url: &str,
    timeout: Duration,
    policy: &ActionSecurityPolicy,
) -> Result<Response, RuntimeActionError> {
    let mut url = Url::parse(url).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("invalid download URL: {source}"),
    })?;
    for redirects in 0_u8..=10 {
        let client = validated_http_client(request, &url, timeout, policy)?;
        let safe_url = safe_http_destination(url.as_str());
        let response =
            client
                .get(url.clone())
                .send()
                .map_err(|source| RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!(
                        "download request {safe_url} failed: {}",
                        sanitized_http_error(&source, url.as_str(), &safe_url)
                    ),
                })?;
        let Some(next_url) = redirect_target(request, &url, &response)? else {
            return Ok(response);
        };
        if redirects == 10 {
            return failed(request, "download request exceeded 10 redirects");
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
    if !matches!(url.scheme(), "http" | "https") {
        return failed(request, "HTTP URL scheme must be http or https");
    }
    let mut builder = Client::builder()
        .timeout(timeout)
        .redirect(Policy::none())
        .no_proxy();
    if !policy.allow_private_http_requests {
        let addresses = resolved_destination_addresses(request, url)?;
        if addresses
            .iter()
            .any(|address| is_private_http_address(*address))
        {
            return failed(
                request,
                format!(
                    "HTTP request to private or local network destination {} is blocked because security.policy.allow_private_http_requests is false",
                    safe_http_destination(url.as_str())
                ),
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
    builder
        .build()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to build HTTP client: {source}"),
        })
}

fn resolved_destination_addresses(
    request: &RuntimeActionRequest,
    url: &Url,
) -> Result<Vec<IpAddr>, RuntimeActionError> {
    let port = url
        .port_or_known_default()
        .ok_or_else(|| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: "HTTP URL is missing a port and has no known default".to_owned(),
        })?;
    match url.host() {
        Some(Host::Ipv4(address)) => Ok(vec![IpAddr::V4(address)]),
        Some(Host::Ipv6(address)) => Ok(vec![IpAddr::V6(address)]),
        Some(Host::Domain(host)) => {
            let addresses = (host, port)
                .to_socket_addrs()
                .map_err(|source| RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("failed to resolve HTTP host {host}: {source}"),
                })?
                .map(|address| address.ip())
                .collect::<Vec<_>>();
            if addresses.is_empty() {
                return failed(request, format!("HTTP host {host} did not resolve"));
            }
            Ok(addresses)
        }
        None => failed(request, "HTTP URL is missing a host"),
    }
}

fn is_private_http_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_private_ipv4_address(address),
        IpAddr::V6(address) => is_private_ipv6_address(address),
    }
}

fn is_private_ipv4_address(address: Ipv4Addr) -> bool {
    address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_documentation()
        || address.octets()[0] == 0
        || address.octets()[0] >= 224
        || address.octets() == [169, 254, 169, 254]
}

fn is_private_ipv6_address(address: Ipv6Addr) -> bool {
    address.is_loopback()
        || address.is_unspecified()
        || (address.segments()[0] & 0xfe00) == 0xfc00
        || (address.segments()[0] & 0xffc0) == 0xfe80
        || (address.segments()[0] & 0xff00) == 0xff00
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
    let location = location
        .to_str()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("HTTP redirect location is not valid header text: {source}"),
        })?;
    current_url
        .join(location)
        .map(Some)
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("HTTP redirect location is not a valid URL: {source}"),
        })
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
    destination.push_str(url.path());
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
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "HTTP request body is not valid JSON for its Content-Type header: {source}"
            ),
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
