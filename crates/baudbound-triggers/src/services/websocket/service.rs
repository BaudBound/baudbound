use std::{
    collections::BTreeSet,
    net::{SocketAddr, TcpListener},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread::{self, JoinHandle},
};

use crate::{
    NetworkTriggerAuthenticator, TriggerError, TriggerEvent, TriggerRegistration,
    TriggerServiceDiagnostics,
};

use super::{
    listener::{WebSocketListenerContext, run_listener},
    registry::WebSocketConnectionRegistry,
    route::{WebSocketRoute, parse_routes},
};

pub struct WebSocketService {
    bound_address: Option<SocketAddr>,
    config: Option<WebSocketServiceConfig>,
    handle: Option<JoinHandle<()>>,
    registry: Arc<WebSocketConnectionRegistry>,
    route_count: usize,
    routes: Arc<RwLock<Vec<WebSocketRoute>>>,
    running: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WebSocketServiceConfig {
    pub allow_browser_origins: BTreeSet<String>,
    pub bind: String,
    pub max_connections: usize,
    pub max_message_bytes: usize,
    pub port: u16,
}

impl WebSocketServiceConfig {
    fn bind_address(&self) -> String {
        format!("{}:{}", self.bind, self.port)
    }
}

impl WebSocketService {
    #[must_use]
    pub fn empty(registry: Arc<WebSocketConnectionRegistry>) -> Self {
        Self {
            bound_address: None,
            config: None,
            handle: None,
            registry,
            route_count: 0,
            routes: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_or_reconfigure(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        config: WebSocketServiceConfig,
        sender: SyncSender<TriggerEvent>,
        authenticator: Arc<dyn NetworkTriggerAuthenticator>,
        registry: Arc<WebSocketConnectionRegistry>,
        previous: Option<Self>,
    ) -> Result<Self, TriggerError> {
        let routes = parse_routes(registrations)?;
        if routes.is_empty() {
            drop(previous);
            return Ok(Self::empty(registry));
        }
        if config.max_message_bytes == 0 {
            return Err(config_error("max_message_bytes must be greater than zero"));
        }
        if config.max_connections == 0 {
            return Err(config_error("max_connections must be greater than zero"));
        }

        if let Some(mut service) = previous {
            if service.config.as_ref() == Some(&config) {
                service.replace_routes(routes);
                return Ok(service);
            }
            drop(service);
        }

        Self::start(routes, config, sender, authenticator, registry)
    }

    fn start(
        routes: Vec<WebSocketRoute>,
        config: WebSocketServiceConfig,
        sender: SyncSender<TriggerEvent>,
        authenticator: Arc<dyn NetworkTriggerAuthenticator>,
        registry: Arc<WebSocketConnectionRegistry>,
    ) -> Result<Self, TriggerError> {
        let bind_address = config.bind_address();
        let listener = TcpListener::bind(&bind_address).map_err(|source| {
            TriggerError::Failed(
                "trigger.websocket".to_owned(),
                format!("failed to bind WebSocket listener on {bind_address}: {source}"),
            )
        })?;
        listener.set_nonblocking(true).map_err(|source| {
            TriggerError::Failed(
                "trigger.websocket".to_owned(),
                format!("failed to configure WebSocket listener: {source}"),
            )
        })?;
        let bound_address = listener.local_addr().map_err(|source| {
            TriggerError::Failed(
                "trigger.websocket".to_owned(),
                format!("failed to inspect WebSocket listener address: {source}"),
            )
        })?;

        let running = Arc::new(AtomicBool::new(true));
        let shared_routes = Arc::new(RwLock::new(routes));
        let allow_browser_origins = Arc::new(config.allow_browser_origins.clone());
        let route_count = shared_routes
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len();
        let handle = thread::Builder::new()
            .name("baudbound-websocket-listener".to_owned())
            .spawn({
                let routes = Arc::clone(&shared_routes);
                let running = Arc::clone(&running);
                let registry = Arc::clone(&registry);
                move || {
                    run_listener(
                        listener,
                        WebSocketListenerContext {
                            allow_browser_origins,
                            authenticator,
                            max_connections: config.max_connections,
                            max_message_bytes: config.max_message_bytes,
                            registry,
                            routes,
                            running,
                            sender,
                        },
                    );
                }
            })
            .map_err(|source| {
                TriggerError::Failed(
                    "trigger.websocket".to_owned(),
                    format!("failed to start WebSocket listener thread: {source}"),
                )
            })?;

        Ok(Self {
            bound_address: Some(bound_address),
            config: Some(config),
            handle: Some(handle),
            registry,
            route_count,
            routes: shared_routes,
            running,
        })
    }

    fn replace_routes(&mut self, routes: Vec<WebSocketRoute>) {
        let valid_keys = routes
            .iter()
            .map(WebSocketRoute::key)
            .collect::<BTreeSet<_>>();
        self.route_count = routes.len();
        *self
            .routes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = routes;
        self.registry.close_except(&valid_keys);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.route_count == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.route_count
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        let running = self.running.load(Ordering::Acquire);
        let active = running && self.route_count > 0;
        TriggerServiceDiagnostics {
            running: active,
            state: if active {
                "active"
            } else if self.route_count > 0 {
                "stopped"
            } else {
                "idle"
            },
            summary: format!(
                "{} WebSocket route{} registered; {} of {} connections active",
                self.route_count,
                if self.route_count == 1 { "" } else { "s" },
                self.registry.connection_count(),
                self.config
                    .as_ref()
                    .map_or(0, |config| config.max_connections),
            ),
        }
    }

    #[must_use]
    pub fn bound_address(&self) -> Option<SocketAddr> {
        self.bound_address
    }
}

impl Drop for WebSocketService {
    fn drop(&mut self) {
        if self.handle.is_none() {
            return;
        }
        self.running.store(false, Ordering::Release);
        self.registry.close_all();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn config_error(message: &str) -> TriggerError {
    TriggerError::Failed("trigger.websocket".to_owned(), message.to_owned())
}
