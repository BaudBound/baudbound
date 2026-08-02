use std::{
    convert::Infallible,
    io,
    net::{SocketAddr, TcpListener as StdTcpListener},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use baudbound_triggers::{WebhookRequest, WebhookResponse};
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
    net::TcpListener,
    sync::{Semaphore, oneshot},
    time::timeout,
};

use super::http::ParsedWebhookRequest;

#[cfg(not(test))]
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const BODY_READ_TIMEOUT: Duration = Duration::from_millis(250);
#[cfg(not(test))]
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const HEADER_READ_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_HEADER_BYTES: usize = 32 * 1024;

pub(super) struct IncomingWebhook {
    pub(super) parsed: ParsedWebhookRequest,
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
    incoming: Receiver<IncomingWebhook>,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl WebhookListener {
    pub(super) fn bind(
        address: &str,
        max_connections: usize,
        max_body_bytes: usize,
    ) -> io::Result<Self> {
        let listener = StdTcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let (incoming_sender, incoming) = mpsc::sync_channel(max_connections);
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?;
        let thread = thread::Builder::new()
            .name("baudbound-webhook-listener".to_owned())
            .spawn(move || {
                runtime.block_on(run_listener(
                    listener,
                    incoming_sender,
                    max_connections,
                    max_body_bytes,
                    shutdown_receiver,
                ));
            })?;
        Ok(Self {
            address,
            incoming,
            shutdown: Some(shutdown),
            thread: Some(thread),
        })
    }

    pub(super) fn local_addr(&self) -> SocketAddr {
        self.address
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
    max_connections: usize,
    max_body_bytes: usize,
    mut shutdown: oneshot::Receiver<()>,
) {
    let listener = match TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!("failed to initialize webhook listener: {error}");
            return;
        }
    };
    let connections = Arc::new(Semaphore::new(max_connections));
    loop {
        let accepted = tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => accepted,
        };
        let (stream, peer_address) = match accepted {
            Ok(accepted) => accepted,
            Err(error) => {
                tracing::error!("webhook listener failed to accept a connection: {error}");
                break;
            }
        };
        let Ok(permit) = connections.clone().try_acquire_owned() else {
            tracing::warn!(
                peer = %peer_address,
                "webhook connection rejected because the listener is at capacity"
            );
            drop(stream);
            continue;
        };
        let incoming_sender = incoming_sender.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let service = service_fn(move |request| {
                handle_request(request, incoming_sender.clone(), max_body_bytes)
            });
            let mut builder = http1::Builder::new();
            builder.timer(TokioTimer::new());
            builder.header_read_timeout(HEADER_READ_TIMEOUT);
            builder.max_buf_size(MAX_HEADER_BYTES);
            if let Err(error) = builder
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                tracing::debug!(peer = %peer_address, "webhook connection ended: {error}");
            }
        });
    }
}

async fn handle_request(
    request: Request<Incoming>,
    incoming_sender: SyncSender<IncomingWebhook>,
    max_body_bytes: usize,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let parsed = match parse_request(request, max_body_bytes).await {
        Ok(parsed) => parsed,
        Err(response) => return Ok(response_from_webhook(response)),
    };
    let (response_sender, response_receiver) = oneshot::channel();
    let incoming = IncomingWebhook {
        parsed,
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

async fn parse_request(
    request: Request<Incoming>,
    max_body_bytes: usize,
) -> Result<ParsedWebhookRequest, WebhookResponse> {
    if request
        .headers()
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > max_body_bytes as u64)
    {
        return Err(webhook_text_response(
            413,
            format!("Request body exceeds {max_body_bytes} bytes."),
        ));
    }

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
    let body = timeout(
        BODY_READ_TIMEOUT,
        read_body(request.into_body(), max_body_bytes),
    )
    .await
    .map_err(|_| webhook_text_response(408, "Timed out while reading the request body."))??;
    Ok(ParsedWebhookRequest {
        origin,
        request: WebhookRequest {
            body: String::from_utf8_lossy(&body).into_owned(),
            headers,
            method,
            path_and_query,
        },
        token,
    })
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

async fn read_body(mut body: Incoming, max_body_bytes: usize) -> Result<Vec<u8>, WebhookResponse> {
    let mut bytes = Vec::new();
    while let Some(frame) = body.frame().await {
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
