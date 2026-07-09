use std::collections::BTreeMap;

use baudbound_runtime::RunReport;
use serde_json::{Value, json};

use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics, config_bool,
    config_string, config_u16, is_supported_http_method, split_path_and_query,
    value_object_to_string_map,
};
#[derive(Debug, Clone)]
pub struct WebhookDispatch {
    pub event: TriggerEvent,
    pub fallback_response: WebhookResponse,
    pub wait_for_response: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookRequest {
    pub body: String,
    pub headers: BTreeMap<String, String>,
    pub method: String,
    pub path_and_query: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookResponse {
    pub body: String,
    pub content_type: String,
    pub headers: BTreeMap<String, String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct WebhookService {
    routes: Vec<WebhookRoute>,
}

impl WebhookService {
    pub fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
    ) -> Result<Self, TriggerError> {
        let mut routes = Vec::new();
        for registration in registrations {
            if registration.action_type != "trigger.webhook" {
                continue;
            }
            routes.push(WebhookRoute::from_registration(registration)?);
        }

        Ok(Self { routes })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::active(self.len(), "webhook route")
    }

    #[must_use]
    pub fn dispatch_for_request(&self, request: &WebhookRequest) -> Option<WebhookDispatch> {
        let method = request.method.to_ascii_uppercase();
        let (path, query) = split_path_and_query(&request.path_and_query);

        self.routes
            .iter()
            .find(|route| route.method == method && route.path == path)
            .map(|route| WebhookDispatch {
                event: TriggerEvent {
                    node_id: route.registration.node_id.clone(),
                    payload: webhook_payload(route, request, &path, &query),
                    script_id: route.registration.script_id.clone(),
                },
                fallback_response: route.fallback_response.clone(),
                wait_for_response: route.wait_for_response,
            })
    }

    #[must_use]
    pub fn response_for_report(
        &self,
        dispatch: &WebhookDispatch,
        report: &RunReport,
    ) -> WebhookResponse {
        if !dispatch.wait_for_response {
            return dispatch.fallback_response.clone();
        }

        response_from_report(&dispatch.event.node_id, report)
            .unwrap_or_else(|| dispatch.fallback_response.clone())
    }
}

#[derive(Debug, Clone)]
struct WebhookRoute {
    fallback_response: WebhookResponse,
    method: String,
    path: String,
    registration: TriggerRegistration,
    wait_for_response: bool,
}

impl WebhookRoute {
    fn from_registration(registration: TriggerRegistration) -> Result<Self, TriggerError> {
        let hook_name = registration
            .config
            .get("hookName")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                TriggerError::Failed(
                    registration.node_id.clone(),
                    "webhook trigger must define hookName".to_owned(),
                )
            })?;
        let method = registration
            .config
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("POST")
            .trim()
            .to_ascii_uppercase();
        if !is_supported_http_method(&method) {
            return Err(TriggerError::Failed(
                registration.node_id.clone(),
                format!("unsupported webhook method {method:?}"),
            ));
        }

        Ok(Self {
            fallback_response: WebhookResponse {
                body: config_string(&registration.config, "timeoutResponseBody")
                    .unwrap_or_else(|| r#"{ "ok": true }"#.to_owned()),
                content_type: config_string(&registration.config, "timeoutResponseContentType")
                    .unwrap_or_else(|| "application/json".to_owned()),
                headers: BTreeMap::new(),
                status_code: config_u16(&registration.config, "timeoutResponseStatus", 200),
            },
            method,
            path: format!("/events/{hook_name}"),
            wait_for_response: config_bool(&registration.config, "waitForResponse"),
            registration,
        })
    }
}

fn webhook_payload(
    route: &WebhookRoute,
    request: &WebhookRequest,
    path: &str,
    query: &BTreeMap<String, String>,
) -> Value {
    let json_body = serde_json::from_str::<Value>(&request.body).unwrap_or_else(|_| json!({}));
    json!({
        "body": request.body,
        "headers": request.headers,
        "json": json_body,
        "method": request.method.to_ascii_uppercase(),
        "path": path,
        "query": query,
        "response": {
            "waiting": route.wait_for_response,
        },
        "trigger_id": route.registration.node_id,
    })
}

fn response_from_report(trigger_node_id: &str, report: &RunReport) -> Option<WebhookResponse> {
    let mut response_prefixes = report
        .variables
        .iter()
        .filter_map(|(key, value)| {
            let prefix = key.strip_suffix(".trigger_id")?;
            (value.as_str() == Some(trigger_node_id)).then_some(prefix.to_owned())
        })
        .collect::<Vec<_>>();
    response_prefixes.sort();

    for prefix in response_prefixes {
        let sent = report
            .variables
            .get(&format!("{prefix}.sent"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !sent {
            continue;
        }

        return Some(WebhookResponse {
            body: report
                .variables
                .get(&format!("{prefix}.body"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            content_type: report
                .variables
                .get(&format!("{prefix}.content_type"))
                .and_then(Value::as_str)
                .unwrap_or("text/plain")
                .to_owned(),
            headers: value_object_to_string_map(report.variables.get(&format!("{prefix}.headers"))),
            status_code: report
                .variables
                .get(&format!("{prefix}.status_code"))
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| (100..=599).contains(value))
                .unwrap_or(200),
        });
    }

    None
}
