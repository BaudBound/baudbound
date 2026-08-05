use std::{
    collections::BTreeSet,
    convert::Infallible,
    io,
    net::{SocketAddr, TcpListener as StdTcpListener},
    sync::{
        Arc, Mutex, RwLock,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use baudbound_runtime::ResourceLimit;
use baudbound_triggers::{
    ConnectionGate, ConnectionPermit, NetworkTriggerAuthenticationError,
    NetworkTriggerAuthenticator, NetworkTriggerKind, PreAuthRateLimit, PreAuthRateLimiter,
    WebhookDispatch, WebhookRequest, WebhookResponse, WebhookService,
};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response, StatusCode,
    body::Incoming,
    header::{AUTHORIZATION, HeaderName, HeaderValue},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinSet,
    time::timeout,
};

use super::http::{ParsedWebhookRequest, preflight_response};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WebhookListenerConfig {
    pub(super) bind_address: String,
    pub(super) body_read_progress_timeout_ms: ResourceLimit,
    pub(super) body_read_timeout_ms: ResourceLimit,
    pub(super) header_read_timeout_ms: ResourceLimit,
    pub(super) max_body_bytes: usize,
    pub(super) max_connections: usize,
    pub(super) max_header_bytes: ResourceLimit,
    pub(super) max_unauthenticated_connections: ResourceLimit,
    pub(super) pre_auth_requests_per_minute_global: ResourceLimit,
    pub(super) pre_auth_requests_per_minute_per_address: ResourceLimit,
    pub(super) pre_auth_timeout_ms: ResourceLimit,
}

#[derive(Clone)]
struct WebhookAdmission {
    allow_browser_origins: BTreeSet<String>,
    authenticator: Arc<dyn NetworkTriggerAuthenticator>,
    service: WebhookService,
}

pub(super) struct IncomingWebhook {
    pub(super) dispatch: WebhookDispatch,
    pub(super) origin: Option<String>,
    pub(super) response: WebhookResponseSender,
}

pub(super) struct WebhookResponseSender {
    sender: Option<oneshot::Sender<WebhookResponse>>,
}

impl WebhookResponseSender {
    pub(super) fn send(mut self, response: WebhookResponse) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(response);
        }
    }
}

pub(super) struct WebhookListener {
    address: SocketAddr,
    admission: Arc<RwLock<WebhookAdmission>>,
    config: WebhookListenerConfig,
    incoming: Receiver<IncomingWebhook>,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl WebhookListener {
    pub(super) fn bind(
        config: WebhookListenerConfig,
        service: WebhookService,
        authenticator: Arc<dyn NetworkTriggerAuthenticator>,
        allow_browser_origins: BTreeSet<String>,
    ) -> io::Result<Self> {
        http1_header_buffer_size(config.max_header_bytes)?;
        let listener = StdTcpListener::bind(&config.bind_address)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let (incoming_sender, incoming) = mpsc::sync_channel(config.max_connections);
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let admission = Arc::new(RwLock::new(WebhookAdmission {
            allow_browser_origins,
            authenticator,
            service,
        }));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;
        let listener_config = config.clone();
        let listener_admission = Arc::clone(&admission);
        let thread = thread::Builder::new()
            .name("baudbound-webhook-listener".to_owned())
            .spawn(move || {
                runtime.block_on(run_listener(
                    listener,
                    incoming_sender,
                    listener_config,
                    listener_admission,
                    shutdown_receiver,
                ));
            })?;
        Ok(Self {
            address,
            admission,
            config,
            incoming,
            shutdown: Some(shutdown),
            thread: Some(thread),
        })
    }

    pub(super) fn restart(
        &mut self,
        config: WebhookListenerConfig,
        service: WebhookService,
        authenticator: Arc<dyn NetworkTriggerAuthenticator>,
        allow_browser_origins: BTreeSet<String>,
    ) -> io::Result<()> {
        if self.config == config {
            self.replace_admission(service, authenticator, allow_browser_origins);
            return Ok(());
        }

        let next_admission = WebhookAdmission {
            allow_browser_origins,
            authenticator,
            service,
        };
        if self.config.bind_address != config.bind_address {
            let replacement = Self::bind(
                config,
                next_admission.service,
                next_admission.authenticator,
                next_admission.allow_browser_origins,
            )?;
            *self = replacement;
            return Ok(());
        }

        let previous_config = self.config.clone();
        let previous_admission = self.admission_snapshot();
        self.shutdown();
        match Self::bind(
            config,
            next_admission.service,
            next_admission.authenticator,
            next_admission.allow_browser_origins,
        ) {
            Ok(replacement) => {
                *self = replacement;
                Ok(())
            }
            Err(source) => match Self::bind(
                previous_config,
                previous_admission.service,
                previous_admission.authenticator,
                previous_admission.allow_browser_origins,
            ) {
                Ok(previous) => {
                    *self = previous;
                    Err(source)
                }
                Err(rollback) => Err(io::Error::new(
                    source.kind(),
                    format!(
                        "failed to bind replacement webhook listener: {source}; failed to restore previous listener: {rollback}"
                    ),
                )),
            },
        }
    }

    pub(super) fn local_addr(&self) -> SocketAddr {
        self.address
    }

    pub(super) fn matches_configuration(&self, config: &WebhookListenerConfig) -> bool {
        &self.config == config
    }

    pub(super) fn replace_admission(
        &self,
        service: WebhookService,
        authenticator: Arc<dyn NetworkTriggerAuthenticator>,
        allow_browser_origins: BTreeSet<String>,
    ) {
        *self
            .admission
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = WebhookAdmission {
            allow_browser_origins,
            authenticator,
            service,
        };
    }

    pub(super) fn recv_timeout(
        &self,
        duration: Duration,
    ) -> Result<Option<IncomingWebhook>, mpsc::RecvTimeoutError> {
        match self.incoming.recv_timeout(duration) {
            Ok(request) => Ok(Some(request)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(error @ mpsc::RecvTimeoutError::Disconnected) => Err(error),
        }
    }

    fn admission_snapshot(&self) -> WebhookAdmission {
        self.admission
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn shutdown(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("webhook listener thread panicked while stopping");
        }
    }
}

impl Drop for WebhookListener {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn run_listener(
    listener: StdTcpListener,
    incoming_sender: SyncSender<IncomingWebhook>,
    config: WebhookListenerConfig,
    admission: Arc<RwLock<WebhookAdmission>>,
    mut shutdown: oneshot::Receiver<()>,
) {
    let listener = match TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!("failed to initialize webhook listener: {error}");
            return;
        }
    };
    let authenticated_connections = Arc::new(ConnectionGate::new(ResourceLimit::limited(
        u64::try_from(config.max_connections).unwrap_or(u64::MAX),
    )));
    let unauthenticated_connections =
        Arc::new(ConnectionGate::new(config.max_unauthenticated_connections));
    let authentication_workers =
        Arc::new(ConnectionGate::new(config.max_unauthenticated_connections));
    let pre_auth_rate_limiter = PreAuthRateLimiter::per_minute(
        config.pre_auth_requests_per_minute_global,
        config.pre_auth_requests_per_minute_per_address,
    );
    let mut connections = JoinSet::new();

    loop {
        let accepted = tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => accepted,
            completed = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = completed {
                    tracing::warn!("webhook connection task failed: {error}");
                }
                continue;
            }
        };
        let (stream, peer_address) = match accepted {
            Ok(accepted) => accepted,
            Err(error) => {
                tracing::error!("webhook listener failed to accept a connection: {error}");
                break;
            }
        };
        if let Err(limit) = pre_auth_rate_limiter.check(peer_address.ip()) {
            let message = match limit {
                PreAuthRateLimit::Address => {
                    "Webhook pre-authentication rate limit reached for this address."
                }
                PreAuthRateLimit::Global => "Webhook pre-authentication rate limit reached.",
            };
            reject_connection(stream, StatusCode::TOO_MANY_REQUESTS, message).await;
            continue;
        }
        let Some(pre_auth_permit) = unauthenticated_connections.try_acquire() else {
            reject_connection(
                stream,
                StatusCode::SERVICE_UNAVAILABLE,
                "Webhook pre-authentication connection limit reached.",
            )
            .await;
            continue;
        };

        let incoming_sender = incoming_sender.clone();
        let authenticated_connections = Arc::clone(&authenticated_connections);
        let authentication_workers = Arc::clone(&authentication_workers);
        let admission = Arc::clone(&admission);
        let request_config = config.clone();
        connections.spawn(async move {
            let permit = Arc::new(Mutex::new(Some(pre_auth_permit)));
            let service = service_fn(move |request| {
                let pre_auth_permit = permit
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                handle_request(
                    request,
                    incoming_sender.clone(),
                    Arc::clone(&admission),
                    Arc::clone(&authenticated_connections),
                    Arc::clone(&authentication_workers),
                    request_config.clone(),
                    pre_auth_permit,
                )
            });
            let mut builder = http1::Builder::new();
            builder.timer(TokioTimer::new());
            builder.keep_alive(false);
            if let Some(milliseconds) = config.header_read_timeout_ms.value() {
                builder.header_read_timeout(Duration::from_millis(milliseconds));
            }
            builder.max_buf_size(
                http1_header_buffer_size(config.max_header_bytes)
                    .expect("validated webhook header limit must remain valid"),
            );
            if let Err(error) = builder
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                tracing::debug!(peer = %peer_address, "webhook connection ended: {error}");
            }
        });
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result
            && !error.is_cancelled()
        {
            tracing::warn!("webhook connection task failed during shutdown: {error}");
        }
    }
}

pub(super) fn http1_header_buffer_size(limit: ResourceLimit) -> io::Result<usize> {
    const MINIMUM_HTTP1_BUFFER_BYTES: usize = 8 * 1024;
    let bytes = match limit.value() {
        Some(bytes) => usize::try_from(bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "webhook header limit does not fit this platform",
            )
        })?,
        None => usize::MAX,
    };
    if bytes < MINIMUM_HTTP1_BUFFER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("webhook header limit must be at least {MINIMUM_HTTP1_BUFFER_BYTES} bytes"),
        ));
    }
    Ok(bytes)
}

async fn handle_request(
    request: Request<Incoming>,
    incoming_sender: SyncSender<IncomingWebhook>,
    admission: Arc<RwLock<WebhookAdmission>>,
    authenticated_connections: Arc<ConnectionGate>,
    authentication_workers: Arc<ConnectionGate>,
    config: WebhookListenerConfig,
    pre_auth_permit: Option<ConnectionPermit>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let Some(pre_auth_permit) = pre_auth_permit else {
        return Ok(text_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Webhook connection cannot process another request.",
        ));
    };

    let metadata = request_metadata(&request);
    let admission = admission
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if metadata.request.method.eq_ignore_ascii_case("OPTIONS") {
        let requested_method = metadata
            .request
            .headers
            .get("access-control-request-method")
            .map(String::as_str)
            .unwrap_or_default();
        if admission
            .service
            .route_target(requested_method, &metadata.request.path_and_query)
            .is_none()
        {
            return Ok(text_response(
                StatusCode::NOT_FOUND,
                "Webhook route not found.",
            ));
        }
        let response = preflight_response(&metadata, &admission.allow_browser_origins)
            .unwrap_or_else(|| webhook_text_response(400, "Invalid browser preflight request."));
        return Ok(response_from_webhook(response));
    }
    if let Some(origin) = metadata.origin.as_deref()
        && !admission.allow_browser_origins.contains(origin)
    {
        return Ok(text_response(
            StatusCode::FORBIDDEN,
            "Browser origin is not allowed.",
        ));
    }
    let Some(target) = admission
        .service
        .route_target(&metadata.request.method, &metadata.request.path_and_query)
    else {
        return Ok(text_response(
            StatusCode::NOT_FOUND,
            "Webhook route not found.",
        ));
    };

    let authenticator = Arc::clone(&admission.authenticator);
    let token = metadata.token.clone();
    let Some(authentication_worker_permit) = authentication_workers.try_acquire() else {
        return Ok(text_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Webhook authentication capacity is temporarily exhausted.",
        ));
    };
    let authentication = tokio::task::spawn_blocking(move || {
        let _authentication_worker_permit = authentication_worker_permit;
        authenticator.authenticate(
            &target.script_id,
            &target.node_id,
            NetworkTriggerKind::Webhook,
            token.as_deref(),
        )
    });
    let authentication = if let Some(milliseconds) = config.pre_auth_timeout_ms.value() {
        match timeout(Duration::from_millis(milliseconds), authentication).await {
            Ok(result) => result,
            Err(_) => {
                return Ok(text_response(
                    StatusCode::REQUEST_TIMEOUT,
                    "Webhook authentication timed out.",
                ));
            }
        }
    } else {
        authentication.await
    };
    match authentication {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Ok(response_from_webhook(authentication_error_response(error))),
        Err(error) => {
            tracing::error!("webhook authentication worker failed: {error}");
            return Ok(text_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Webhook authentication is unavailable.",
            ));
        }
    }

    let Some(_authenticated_permit) = authenticated_connections.try_acquire() else {
        return Ok(text_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Webhook authenticated connection limit reached.",
        ));
    };
    drop(pre_auth_permit);

    if request
        .headers()
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > config.max_body_bytes as u64)
    {
        return Ok(text_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("Request body exceeds {} bytes.", config.max_body_bytes),
        ));
    }
    let body = match read_body(
        request.into_body(),
        config.max_body_bytes,
        config.body_read_progress_timeout_ms,
        config.body_read_timeout_ms,
    )
    .await
    {
        Ok(body) => body,
        Err(response) => return Ok(response_from_webhook(response)),
    };
    let parsed = ParsedWebhookRequest {
        origin: metadata.origin,
        request: WebhookRequest {
            body: String::from_utf8_lossy(&body).into_owned(),
            ..metadata.request
        },
        token: metadata.token,
    };
    let Some(dispatch) = admission.service.dispatch_for_request(&parsed.request) else {
        return Ok(text_response(
            StatusCode::NOT_FOUND,
            "Webhook route not found.",
        ));
    };
    let (response_sender, response_receiver) = oneshot::channel();
    let incoming = IncomingWebhook {
        dispatch,
        origin: parsed.origin,
        response: WebhookResponseSender {
            sender: Some(response_sender),
        },
    };
    match incoming_sender.try_send(incoming) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            return Ok(text_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Webhook listener is at capacity. Try again later.",
            ));
        }
        Err(TrySendError::Disconnected(_)) => {
            return Ok(text_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Webhook service is stopping or reloading.",
            ));
        }
    }
    Ok(match response_receiver.await {
        Ok(response) => response_from_webhook(response),
        Err(_) => text_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Webhook service is stopping or reloading.",
        ),
    })
}

fn request_metadata(request: &Request<Incoming>) -> ParsedWebhookRequest {
    let method = request.method().as_str().to_owned();
    let path_and_query = request
        .uri()
        .path_and_query()
        .map_or_else(|| request.uri().path().to_owned(), ToString::to_string);
    let token = bearer_token(request.headers());
    let origin = header_value(request.headers(), "origin");
    let headers = request
        .headers()
        .iter()
        .filter(|(name, _)| *name != AUTHORIZATION)
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.trim().to_owned()))
        })
        .collect();
    ParsedWebhookRequest {
        origin,
        request: WebhookRequest {
            body: String::new(),
            headers,
            method,
            path_and_query,
        },
        token,
    }
}

fn bearer_token(headers: &hyper::HeaderMap) -> Option<String> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let authorization = values.next()?.to_str().ok()?.trim();
    if values.next().is_some() {
        return None;
    }

    let mut parts = authorization.split_ascii_whitespace();
    match (parts.next(), parts.next(), parts.next()) {
        (Some(scheme), Some(token), None) if scheme.eq_ignore_ascii_case("bearer") => {
            Some(token.to_owned())
        }
        _ => None,
    }
}

async fn read_body(
    body: Incoming,
    max_body_bytes: usize,
    progress_timeout: ResourceLimit,
    total_timeout: ResourceLimit,
) -> Result<Vec<u8>, WebhookResponse> {
    let read = read_body_with_progress(body, max_body_bytes, progress_timeout);
    if let Some(milliseconds) = total_timeout.value() {
        timeout(Duration::from_millis(milliseconds), read)
            .await
            .map_err(|_| webhook_text_response(408, "Timed out while reading the request body."))?
    } else {
        read.await
    }
}

async fn read_body_with_progress(
    mut body: Incoming,
    max_body_bytes: usize,
    progress_timeout: ResourceLimit,
) -> Result<Vec<u8>, WebhookResponse> {
    let mut bytes = Vec::new();
    loop {
        let next = if let Some(milliseconds) = progress_timeout.value() {
            timeout(Duration::from_millis(milliseconds), body.frame())
                .await
                .map_err(|_| webhook_text_response(408, "Request body stopped making progress."))?
        } else {
            body.frame().await
        };
        let Some(frame) = next else {
            break;
        };
        let frame = frame.map_err(|error| {
            webhook_text_response(400, format!("Failed to read request body: {error}"))
        })?;
        let Ok(data) = frame.into_data() else {
            continue;
        };
        if bytes.len().saturating_add(data.len()) > max_body_bytes {
            return Err(webhook_text_response(
                413,
                format!("Request body exceeds {max_body_bytes} bytes."),
            ));
        }
        bytes.extend_from_slice(&data);
    }
    Ok(bytes)
}

fn header_value(headers: &hyper::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn authentication_error_response(error: NetworkTriggerAuthenticationError) -> WebhookResponse {
    match error {
        NetworkTriggerAuthenticationError::MissingToken => {
            webhook_text_response(401, "Webhook Bearer authorization is required.")
        }
        NetworkTriggerAuthenticationError::InvalidToken => {
            webhook_text_response(403, "Webhook token is invalid.")
        }
        NetworkTriggerAuthenticationError::Unavailable(error) => {
            tracing::error!("webhook authentication state is unavailable: {error}");
            webhook_text_response(503, "Webhook authentication is unavailable.")
        }
    }
}

fn response_from_webhook(webhook_response: WebhookResponse) -> Response<Full<Bytes>> {
    let status = StatusCode::from_u16(webhook_response.status_code)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = Response::new(Full::new(Bytes::from(webhook_response.body)));
    *response.status_mut() = status;
    if let Ok(value) = HeaderValue::from_str(&webhook_response.content_type) {
        response
            .headers_mut()
            .insert(hyper::header::CONTENT_TYPE, value);
    }
    for (name, value) in webhook_response.headers {
        let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(&value) else {
            continue;
        };
        response.headers_mut().insert(name, value);
    }
    response
}

fn text_response(status: StatusCode, body: impl Into<String>) -> Response<Full<Bytes>> {
    response_from_webhook(webhook_text_response(status.as_u16(), body))
}

fn webhook_text_response(status_code: u16, body: impl Into<String>) -> WebhookResponse {
    WebhookResponse {
        body: body.into(),
        content_type: "text/plain".to_owned(),
        headers: Default::default(),
        status_code,
    }
}

async fn reject_connection(mut stream: TcpStream, status: StatusCode, body: &str) {
    let reason = status.canonical_reason().unwrap_or("Rejected");
    let response = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        status.as_u16(),
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}
