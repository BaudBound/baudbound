use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Value, json};
use url::Url;

use crate::required_string;

pub(crate) fn parse_url_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let input = required_string(request, "url")?;
    let parsed = Url::parse(&input).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!("invalid absolute URL: {source}"),
    })?;

    let query_parameters = parsed
        .query_pairs()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect::<Vec<_>>();

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            (
                "protocol".to_owned(),
                Value::String(parsed.scheme().to_owned()),
            ),
            (
                "host".to_owned(),
                Value::String(parsed.host_str().unwrap_or_default().to_owned()),
            ),
            (
                "port".to_owned(),
                Value::String(
                    parsed
                        .port()
                        .map(|port| port.to_string())
                        .unwrap_or_default(),
                ),
            ),
            ("path".to_owned(), Value::String(parsed.path().to_owned())),
            (
                "query".to_owned(),
                Value::String(parsed.query().unwrap_or_default().to_owned()),
            ),
            (
                "query_parameters".to_owned(),
                Value::Array(query_parameters),
            ),
            (
                "fragment".to_owned(),
                Value::String(parsed.fragment().unwrap_or_default().to_owned()),
            ),
        ]),
    })
}
