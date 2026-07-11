use base64::{Engine as _, engine::general_purpose};
use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use regex::Regex;
use serde_json::{Map, Value};

use crate::{
    config_string, config_usize, failed, optional_config_usize, value_kind, value_to_string,
};

pub(crate) fn text_format_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let operation =
        config_string(&request.config, "operation").unwrap_or_else(|| "template".to_owned());
    let input = config_string(&request.config, "input").unwrap_or_default();
    let template = config_string(&request.config, "template").unwrap_or_default();
    let search = config_string(&request.config, "search").unwrap_or_default();
    let replacement = config_string(&request.config, "replacement").unwrap_or_default();
    let delimiter = config_string(&request.config, "delimiter").unwrap_or_else(|| ",".to_owned());
    let pad = config_string(&request.config, "pad").unwrap_or_else(|| " ".to_owned());

    let (text, items) = match operation.as_str() {
        "template" => (template, Vec::new()),
        "trim" => (input.trim().to_owned(), Vec::new()),
        "uppercase" => (input.to_uppercase(), Vec::new()),
        "lowercase" => (input.to_lowercase(), Vec::new()),
        "replace" => (input.replace(&search, &replacement), Vec::new()),
        "regex_replace" => {
            let regex = Regex::new(&search).map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("invalid regex pattern: {source}"),
            })?;
            (
                regex.replace_all(&input, replacement.as_str()).to_string(),
                Vec::new(),
            )
        }
        "split" => (
            String::new(),
            input
                .split(&delimiter)
                .map(|item| Value::String(item.to_owned()))
                .collect(),
        ),
        "join" => {
            let items = parse_items(request)?;
            let text = items
                .iter()
                .map(value_to_string)
                .collect::<Vec<_>>()
                .join(&delimiter);
            (text, items)
        }
        "substring" => {
            let start = config_usize(&request.config, "start", 0);
            let length = optional_config_usize(&request.config, "length");
            (substring_by_chars(&input, start, length), Vec::new())
        }
        "pad_start" => (
            pad_text(
                &input,
                config_usize(&request.config, "targetLength", input.chars().count()),
                &pad,
                true,
            ),
            Vec::new(),
        ),
        "pad_end" => (
            pad_text(
                &input,
                config_usize(&request.config, "targetLength", input.chars().count()),
                &pad,
                false,
            ),
            Vec::new(),
        ),
        "url_encode" => (encode_uri_component(&input), Vec::new()),
        "url_decode" => (
            decode_uri_component(&input).map_err(|message| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message,
            })?,
            Vec::new(),
        ),
        "base64_encode" => (
            general_purpose::STANDARD.encode(input.as_bytes()),
            Vec::new(),
        ),
        "base64_decode" => {
            let bytes = general_purpose::STANDARD
                .decode(input.trim())
                .map_err(|source| RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("invalid base64 input: {source}"),
                })?;
            let text = String::from_utf8(bytes).map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("decoded base64 is not valid UTF-8: {source}"),
            })?;
            (text, Vec::new())
        }
        "json_escape" => (
            serde_json::to_string(&input).map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to JSON escape input: {source}"),
            })?,
            Vec::new(),
        ),
        "json_unescape" => {
            let value = serde_json::from_str::<Value>(&input).map_err(|source| {
                RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("failed to JSON unescape input: {source}"),
                }
            })?;
            let text = match value {
                Value::String(value) => value,
                value => {
                    serde_json::to_string(&value).map_err(|source| RuntimeActionError::Failed {
                        action_type: request.action_type.clone(),
                        message: format!("failed to serialize JSON value: {source}"),
                    })?
                }
            };
            (text, Vec::new())
        }
        _ => {
            return failed(
                request,
                format!("unsupported text transform operation {operation}"),
            );
        }
    };

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("text".to_owned(), Value::String(text)),
            ("items".to_owned(), Value::Array(items)),
        ]),
    })
}

fn parse_items(request: &RuntimeActionRequest) -> Result<Vec<Value>, RuntimeActionError> {
    match request.config.get("items") {
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(Value::String(items)) => {
            let parsed = serde_json::from_str::<Value>(items).map_err(|source| {
                RuntimeActionError::Failed {
                    action_type: request.action_type.clone(),
                    message: format!("join items must be a JSON array: {source}"),
                }
            })?;
            match parsed {
                Value::Array(items) => Ok(items),
                _ => failed(request, "join items must be a JSON array"),
            }
        }
        Some(other) => failed(
            request,
            format!("join items must be a list, found {}", value_kind(other)),
        ),
        None => Ok(Vec::new()),
    }
}

fn substring_by_chars(input: &str, start: usize, length: Option<usize>) -> String {
    let chars = input.chars().skip(start);
    match length {
        Some(length) => chars.take(length).collect(),
        None => chars.collect(),
    }
}

fn pad_text(input: &str, target_length: usize, pad: &str, start: bool) -> String {
    let current_length = input.chars().count();
    if current_length >= target_length || pad.is_empty() {
        return input.to_owned();
    }

    let missing = target_length - current_length;
    let repeated = pad.chars().cycle().take(missing).collect::<String>();
    if start {
        format!("{repeated}{input}")
    } else {
        format!("{input}{repeated}")
    }
}

fn encode_uri_component(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(encoded, "%{byte:02X}").expect("writing to a string cannot fail");
        }
    }
    encoded
}

fn decode_uri_component(input: &str) -> Result<String, String> {
    let input = input.as_bytes();
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        if input[index] != b'%' {
            decoded.push(input[index]);
            index += 1;
            continue;
        }

        if index + 2 >= input.len() {
            return Err("invalid URL encoded input: incomplete percent escape".to_owned());
        }
        let high = decode_hex_digit(input[index + 1]);
        let low = decode_hex_digit(input[index + 2]);
        let (Some(high), Some(low)) = (high, low) else {
            return Err(
                "invalid URL encoded input: percent escape must contain two hexadecimal digits"
                    .to_owned(),
            );
        };
        decoded.push((high << 4) | low);
        index += 3;
    }

    String::from_utf8(decoded)
        .map_err(|source| format!("invalid URL encoded UTF-8 input: {source}"))
}

fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
