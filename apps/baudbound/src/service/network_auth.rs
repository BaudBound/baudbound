use std::net::IpAddr;

use anyhow::{Result, bail};
use baudbound_core::{RunnerCore, TriggerRegistration};
use baudbound_storage::{NetworkTriggerType, SqliteRunnerStore, TriggerAuthentication};
use baudbound_triggers::{
    NetworkTriggerAuthenticationError, NetworkTriggerAuthenticator, NetworkTriggerKind,
};

pub(super) struct RunnerNetworkTriggerAuthenticator {
    core: RunnerCore,
    store: SqliteRunnerStore,
}

impl RunnerNetworkTriggerAuthenticator {
    pub(super) fn new(core: &RunnerCore, store: &SqliteRunnerStore) -> Self {
        Self {
            core: core.clone(),
            store: store.clone(),
        }
    }
}

impl NetworkTriggerAuthenticator for RunnerNetworkTriggerAuthenticator {
    fn authenticate(
        &self,
        script_id: &str,
        node_id: &str,
        trigger_kind: NetworkTriggerKind,
        provided_token: Option<&str>,
    ) -> Result<(), NetworkTriggerAuthenticationError> {
        let trigger_type = match trigger_kind {
            NetworkTriggerKind::Webhook => NetworkTriggerType::Webhook,
            NetworkTriggerKind::WebSocket => NetworkTriggerType::Websocket,
        };
        match self.core.authenticate_network_trigger(
            &self.store,
            script_id,
            node_id,
            trigger_type,
            provided_token,
        ) {
            Ok(TriggerAuthentication::Authenticated | TriggerAuthentication::Disabled) => Ok(()),
            Ok(TriggerAuthentication::InvalidToken) => {
                Err(NetworkTriggerAuthenticationError::InvalidToken)
            }
            Ok(TriggerAuthentication::MissingToken) => {
                Err(NetworkTriggerAuthenticationError::MissingToken)
            }
            Err(error) => Err(NetworkTriggerAuthenticationError::Unavailable(
                error.to_string(),
            )),
        }
    }
}

pub(super) fn validate_listener_exposure(
    core: &RunnerCore,
    store: &SqliteRunnerStore,
    registrations: &[TriggerRegistration],
    trigger_kind: NetworkTriggerKind,
    bind: &str,
    port: u16,
    allow_unauthenticated_public_bind: bool,
) -> Result<()> {
    if is_loopback_bind(bind) {
        return Ok(());
    }
    let action_type = match trigger_kind {
        NetworkTriggerKind::Webhook => "trigger.webhook",
        NetworkTriggerKind::WebSocket => "trigger.websocket",
    };
    let listener_name = match trigger_kind {
        NetworkTriggerKind::Webhook => "webhook",
        NetworkTriggerKind::WebSocket => "WebSocket",
    };
    let mut unprotected = Vec::new();
    for registration in registrations
        .iter()
        .filter(|registration| registration.action_type == action_type)
    {
        let trigger_type = match trigger_kind {
            NetworkTriggerKind::Webhook => NetworkTriggerType::Webhook,
            NetworkTriggerKind::WebSocket => NetworkTriggerType::Websocket,
        };
        let status = core
            .list_trigger_auth(store, &registration.script_id)?
            .into_iter()
            .find(|status| {
                status.node_id == registration.node_id && status.trigger_type == trigger_type
            });
        if !status.is_some_and(|status| status.auth_enabled) {
            unprotected.push(format!(
                "{}:{}",
                registration.script_id, registration.node_id
            ));
        }
    }
    if unprotected.is_empty() {
        return Ok(());
    }
    if allow_unauthenticated_public_bind {
        tracing::warn!(
            "starting public {listener_name} listener on {bind}:{port} with authentication disabled for: {}",
            unprotected.join(", ")
        );
        return Ok(());
    }
    bail!(
        "refusing to start public {listener_name} listener on {bind}:{port} because authentication is disabled or unavailable for {} trigger(s): {}. Enable authentication, bind to a loopback address, or explicitly allow unauthenticated public bind in the runner config",
        unprotected.len(),
        unprotected.join(", ")
    )
}

fn is_loopback_bind(bind: &str) -> bool {
    let bind = bind.trim();
    bind.eq_ignore_ascii_case("localhost")
        || bind
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::is_loopback_bind;

    #[test]
    fn classifies_listener_bind_addresses_conservatively() {
        assert!(is_loopback_bind("localhost"));
        assert!(is_loopback_bind("127.0.0.1"));
        assert!(is_loopback_bind("127.10.20.30"));
        assert!(is_loopback_bind("::1"));
        assert!(!is_loopback_bind("0.0.0.0"));
        assert!(!is_loopback_bind("::"));
        assert!(!is_loopback_bind("192.168.1.5"));
        assert!(!is_loopback_bind("runner.internal"));
        assert!(!is_loopback_bind("not an address"));
    }
}
