use base64::{Engine as _, engine::general_purpose};
use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use regex::Regex;
use serde_json::{Map, Value};

use crate::{config_string, failed, value_kind, value_to_string};

pub(crate) fn text_format_action(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let mut current = request
        .config
        .get("input")
        .cloned()
        .unwrap_or(Value::String(String::new()));
    let operations = request
        .config
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: "text transform requires an operations list".to_owned(),
        })?;
    if operations.is_empty() {
        return failed(request, "text transform requires at least one operation");
    }
    for (index, operation) in operations.iter().enumerate() {
        let config = operation
            .as_object()
            .ok_or_else(|| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("text transform operation {} must be an object", index + 1),
            })?;
        current = apply_operation(current, config).map_err(|error| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("text transform operation {} failed: {error}", index + 1),
        })?;
    }
    let (text, items) = match current {
        Value::Array(items) => (String::new(), items),
        value => (value_to_string(&value), Vec::new()),
    };

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("text".to_owned(), Value::String(text)),
            ("items".to_owned(), Value::Array(items)),
        ]),
    })
}

fn apply_operation(current: Value, config: &Map<String, Value>) -> Result<Value, String> {
    let operation =
        config_string(config, "operation").ok_or_else(|| "operation is required".to_owned())?;
    if operation == "template" {
        return Ok(config
            .get("template")
            .cloned()
            .unwrap_or(Value::String(String::new())));
    }
    if operation == "join" {
        let Value::Array(items) = current else {
            return Err(format!(
                "join requires a list, found {}",
                value_kind(&current)
            ));
        };
        let delimiter = config_string(config, "delimiter").unwrap_or_default();
        if delimiter.is_empty() {
            return Err("join delimiter is required".to_owned());
        }
        return Ok(Value::String(
            items
                .iter()
                .map(value_to_string)
                .collect::<Vec<_>>()
                .join(&delimiter),
        ));
    }
    let Value::String(input) = current else {
        return Err(format!(
            "{operation} requires text, found {}",
            value_kind(&current)
        ));
    };
    let search = config_string(config, "search").unwrap_or_default();
    let replacement = config_string(config, "replacement").unwrap_or_default();
    let delimiter = config_string(config, "delimiter").unwrap_or_default();
    let pad = config_string(config, "pad").unwrap_or_default();

    match operation.as_str() {
        "trim" => Ok(Value::String(input.trim().to_owned())),
        "uppercase" => Ok(Value::String(input.to_uppercase())),
        "lowercase" => Ok(Value::String(input.to_lowercase())),
        "sentence_case" => Ok(Value::String(sentence_case(&input))),
        "capitalize_words" => Ok(Value::String(capitalize_words(&input))),
        "replace" => {
            if search.is_empty() {
                return Err("search text is required".to_owned());
            }
            Ok(Value::String(input.replace(&search, &replacement)))
        }
        "regex_replace" => {
            if search.is_empty() {
                return Err("search text is required".to_owned());
            }
            validate_portable_regex(&search, &replacement)?;
            Regex::new(&search)
                .map(|regex| {
                    Value::String(regex.replace_all(&input, replacement.as_str()).to_string())
                })
                .map_err(|source| format!("invalid regex pattern: {source}"))
        }
        "split" => {
            if delimiter.is_empty() {
                return Err("split delimiter is required".to_owned());
            }
            Ok(Value::Array(
                input
                    .split(&delimiter)
                    .map(|item| Value::String(item.to_owned()))
                    .collect(),
            ))
        }
        "substring" => Ok(Value::String(substring_by_chars(
            &input,
            required_usize(config, "start")?,
            optional_usize(config, "length")?,
        ))),
        "pad_start" => {
            if pad.is_empty() {
                return Err("pad text is required".to_owned());
            }
            Ok(Value::String(pad_text(
                &input,
                required_usize(config, "targetLength")?,
                &pad,
                true,
            )))
        }
        "pad_end" => {
            if pad.is_empty() {
                return Err("pad text is required".to_owned());
            }
            Ok(Value::String(pad_text(
                &input,
                required_usize(config, "targetLength")?,
                &pad,
                false,
            )))
        }
        "url_encode" => Ok(Value::String(encode_uri_component(&input))),
        "url_decode" => decode_uri_component(&input).map(Value::String),
        "base64_encode" => Ok(Value::String(
            general_purpose::STANDARD.encode(input.as_bytes()),
        )),
        "base64_decode" => {
            let bytes = general_purpose::STANDARD
                .decode(input.trim())
                .map_err(|source| format!("invalid base64 input: {source}"))?;
            String::from_utf8(bytes)
                .map(Value::String)
                .map_err(|source| format!("decoded base64 is not valid UTF-8: {source}"))
        }
        "json_escape" => serde_json::to_string(&input)
            .map(Value::String)
            .map_err(|source| format!("failed to JSON escape input: {source}")),
        "json_unescape" => {
            let value = serde_json::from_str::<Value>(&input)
                .map_err(|source| format!("failed to JSON unescape input: {source}"))?;
            Ok(Value::String(match value {
                Value::String(value) => value,
                value => serde_json::to_string(&value)
                    .map_err(|source| format!("failed to serialize JSON value: {source}"))?,
            }))
        }
        _ => Err(format!("unsupported text transform operation {operation}")),
    }
}

fn required_usize(config: &Map<String, Value>, key: &str) -> Result<usize, String> {
    let value = config
        .get(key)
        .ok_or_else(|| format!("{key} is required"))?;
    parse_usize(value, key)
}

fn optional_usize(config: &Map<String, Value>, key: &str) -> Result<Option<usize>, String> {
    match config.get(key) {
        None => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(value) => parse_usize(value, key).map(Some),
    }
}

fn parse_usize(value: &Value, key: &str) -> Result<usize, String> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    let parsed = match value {
        Value::Number(value) => value
            .as_u64()
            .ok_or_else(|| format!("{key} must be a non-negative safe integer"))?,
        Value::String(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("{key} must be a non-negative safe integer"))?,
        _ => return Err(format!("{key} must be a non-negative safe integer")),
    };
    if parsed > MAX_SAFE_INTEGER {
        return Err(format!(
            "{key} must be a non-negative safe integer no greater than {MAX_SAFE_INTEGER}"
        ));
    }
    usize::try_from(parsed).map_err(|_| format!("{key} is too large for this platform"))
}

fn validate_portable_regex(pattern: &str, replacement: &str) -> Result<(), String> {
    if ["(?=", "(?!", "(?<=", "(?<!", "(?<", "(?P<"]
        .iter()
        .any(|marker| pattern.contains(marker))
    {
        return Err(
            "regular expressions cannot use lookaround or named capture groups because they must work in both the editor and runner"
                .to_owned(),
        );
    }
    if pattern
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'\\' && pair[1].is_ascii_digit() && pair[1] != b'0')
    {
        return Err(
            "regular expressions cannot use backreferences in the search pattern because they must work in both the editor and runner"
                .to_owned(),
        );
    }
    let bytes = replacement.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'$' {
            index += 1;
            continue;
        }
        let Some(next) = bytes.get(index + 1) else {
            return Err("a literal $ in a regex replacement must be written as $$".to_owned());
        };
        if *next == b'$' {
            index += 2;
            continue;
        }
        if next.is_ascii_digit() && *next != b'0' {
            index += 2;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            continue;
        }
        return Err(
            "regex replacements support numbered capture groups such as $1; other $ replacement forms are not portable"
                .to_owned(),
        );
    }
    Ok(())
}

fn substring_by_chars(input: &str, start: usize, length: Option<usize>) -> String {
    let chars = input.chars().skip(start);
    match length {
        Some(length) => chars.take(length).collect(),
        None => chars.collect(),
    }
}

fn sentence_case(input: &str) -> String {
    let mut characters = input.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };

    let mut result = first.to_uppercase().collect::<String>();
    result.push_str(&characters.as_str().to_lowercase());
    result
}

fn capitalize_words(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut waiting_for_first_letter = true;

    for character in input.chars() {
        if character.is_whitespace() {
            waiting_for_first_letter = true;
            result.push(character);
            continue;
        }

        if !character.is_alphabetic() {
            result.push(character);
            continue;
        }

        if waiting_for_first_letter {
            result.extend(character.to_uppercase());
            waiting_for_first_letter = false;
        } else {
            result.extend(character.to_lowercase());
        }
    }

    result
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
