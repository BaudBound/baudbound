use serde_json::{Map, Value};

use crate::runtime::{config_string, value_to_string};

pub(super) fn action_completion_message(
    action_type: &str,
    config: &Map<String, Value>,
    output: &Map<String, Value>,
) -> Option<String> {
    let message = match action_type {
        "action.application.open" => format!(
            "Opened application {} with process ID {}.",
            quoted(output_or_config(
                output,
                config,
                "application_id",
                "application"
            )),
            displayed(output.get("process_id"))
        ),
        "action.beep" => format!(
            "Played a {} Hz beep for {} ms.",
            displayed(
                output
                    .get("frequency_hz")
                    .or_else(|| config.get("frequencyHz"))
            ),
            displayed(
                output
                    .get("duration_ms")
                    .or_else(|| config.get("durationMs"))
            )
        ),
        "action.clipboard.get" => format!(
            "Read {} characters from the clipboard.",
            output
                .get("text")
                .and_then(Value::as_str)
                .map_or(0, |value| value.chars().count())
        ),
        "action.clipboard.set" => format!(
            "Wrote {} bytes to the clipboard.",
            displayed(output.get("bytes"))
        ),
        "action.file.copy" => format!(
            "Copied file {} to {}. Bytes copied: {}.",
            quoted(output_or_config(
                output,
                config,
                "source_path",
                "sourcePath"
            )),
            quoted(output_or_config(
                output,
                config,
                "destination_path",
                "destinationPath"
            )),
            displayed(output.get("bytes"))
        ),
        "action.file.delete" => format!(
            "Deleted file {}.",
            quoted(output_or_config(output, config, "path", "path"))
        ),
        "action.file.download" => format!(
            "Downloaded {} bytes to {}.",
            displayed(output.get("bytes")),
            quoted(output_or_config(output, config, "path", "destinationPath"))
        ),
        "action.file.move" => format!(
            "Moved file {} to {}.",
            quoted(output_or_config(
                output,
                config,
                "source_path",
                "sourcePath"
            )),
            quoted(output_or_config(
                output,
                config,
                "destination_path",
                "destinationPath"
            ))
        ),
        "action.file.read" => format!(
            "Read {} bytes from file {}.",
            displayed(output.get("bytes")),
            quoted(output_or_config(output, config, "path", "path"))
        ),
        "action.file.write" => format!(
            "Wrote {} bytes to file {} using {} mode.",
            displayed(output.get("bytes")),
            quoted(output_or_config(output, config, "path", "path")),
            quoted(output_or_config(output, config, "mode", "mode"))
        ),
        // HTTP already emits request, response, status, timing, header, and body diagnostics.
        "action.http" => return None,
        "action.keyboard" => format!(
            "Sent keyboard input {} for key {}.",
            quoted(output_or_config(
                output,
                config,
                "input_action",
                "inputAction"
            )),
            quoted(output_or_config(output, config, "key", "key"))
        ),
        "action.keyboard.type_text" => {
            format!("Typed {} characters.", displayed(output.get("chars")))
        }
        "action.message_box" => format!(
            "Displayed message box {}. Selected button: {}.",
            quoted(output_or_config(output, config, "title", "title")),
            quoted(output_value(output, "button"))
        ),
        "action.mouse" => {
            let click_type = output
                .get("click_type")
                .or_else(|| config.get("clickType"))
                .map(|value| format!(" with {} click type", quoted(Some(value))))
                .unwrap_or_default();
            format!(
                "Sent mouse input {} for the {} button{}.",
                quoted(output_or_config(
                    output,
                    config,
                    "input_action",
                    "inputAction"
                )),
                displayed(output.get("button").or_else(|| config.get("button"))),
                click_type
            )
        }
        "action.mouse.move" => format!(
            "Moved the mouse to x={}, y={} (relative={}).",
            displayed(output.get("x").or_else(|| config.get("x"))),
            displayed(output.get("y").or_else(|| config.get("y"))),
            displayed(output.get("relative").or_else(|| config.get("relative")))
        ),
        "action.notification" => format!(
            "Displayed notification {} with message {}.",
            quoted(output_or_config(output, config, "title", "title")),
            quoted(output_or_config(output, config, "message", "message"))
        ),
        "action.pixel.get" => format!(
            "Read pixel at x={}, y={}. Color: {}.",
            displayed(output.get("x").or_else(|| config.get("x"))),
            displayed(output.get("y").or_else(|| config.get("y"))),
            displayed(output.get("hex"))
        ),
        "action.process.kill" => format!(
            "Terminated process {} with process ID {}.",
            quoted(output_or_config(output, config, "process_name", "target")),
            displayed(output.get("process_id"))
        ),
        "action.process.run" => format!(
            "Process {} finished with process ID {}, exit code {}, and success={}.",
            quoted(config.get("executable")),
            displayed(output.get("process_id")),
            displayed(output.get("exit_code")),
            displayed(output.get("success"))
        ),
        "action.process.status" => format!(
            "Checked process target {} using {} matching. Running: {}. Process ID: {}.",
            quoted(config.get("target")),
            quoted(config.get("matchMode")),
            displayed(output.get("running")),
            displayed(output.get("process_id"))
        ),
        "action.script.run" => format!(
            "Ran sub-script {}. Child run ID: {}. Status: {}.",
            quoted(config.get("script")),
            quoted(output_value(output, "run_id")),
            displayed(output.get("status"))
        ),
        "action.serial.write" => format!(
            "Wrote {} bytes to serial device {} on port {}.",
            displayed(output.get("bytes")),
            quoted(output_or_config(output, config, "device_id", "deviceId")),
            quoted(output_value(output, "port"))
        ),
        "action.shell" => format!(
            "Shell command finished with process ID {}, exit code {}, and success={}.",
            displayed(output.get("process_id")),
            displayed(output.get("exit_code")),
            displayed(output.get("success"))
        ),
        "action.sound.play" => format!(
            "Finished playing sound from {}.",
            quoted(
                output
                    .get("asset_path")
                    .or_else(|| output.get("file_path"))
                    .or_else(|| config.get("assetPath"))
                    .or_else(|| config.get("filePath"))
            )
        ),
        "action.text.format" => format!(
            "Applied {} text transform operation{}. Result: {}.",
            operation_count(config),
            if operation_count(config) == 1 {
                ""
            } else {
                "s"
            },
            displayed(output.get("text"))
        ),
        "action.url.parse" => format!(
            "Parsed URL. Protocol: {}, host: {}, path: {}, query parameters: {}.",
            quoted(output_value(output, "protocol")),
            quoted(output_value(output, "host")),
            quoted(output_value(output, "path")),
            output
                .get("query_parameters")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        ),
        "action.value.convert" => format!(
            "Converted value to {}. Result: {}.",
            quoted(output_or_config(
                output,
                config,
                "target_type",
                "targetType"
            )),
            displayed(output.get("value"))
        ),
        "action.webhook_response" => format!(
            "Sent webhook response {} with content type {} and {} body bytes.",
            displayed(
                output
                    .get("status_code")
                    .or_else(|| config.get("statusCode"))
            ),
            quoted(output_or_config(
                output,
                config,
                "content_type",
                "contentType"
            )),
            output
                .get("body")
                .or_else(|| config.get("body"))
                .and_then(Value::as_str)
                .map_or(0, str::len)
        ),
        "action.websocket.write" => format!(
            "Sent {} bytes to WebSocket connection {}.",
            displayed(output.get("bytes")),
            quoted(output_or_config(
                output,
                config,
                "connection_id",
                "connectionId"
            ))
        ),
        "action.window.active" => format!(
            "Read the active window. Title: {}. Process ID: {}.",
            quoted(output.get("window_title").or_else(|| output.get("title"))),
            displayed(output.get("process_id"))
        ),
        "action.window.focus" => format!(
            "Focused window target {} using {} matching.",
            quoted(output_or_config(output, config, "target", "target")),
            quoted(output_or_config(output, config, "match_mode", "matchMode"))
        ),
        other => {
            format!(
                "Executed action {other}. Output: {}.",
                diagnostic_value(&Value::Object(output.clone()))
            )
        }
    };
    Some(format!(
        "{message} Inputs: {}. Outputs: {}.",
        diagnostic_map(config),
        diagnostic_map(output)
    ))
}

fn operation_count(config: &Map<String, Value>) -> usize {
    config
        .get("operations")
        .and_then(Value::as_array)
        .map_or_else(
            || usize::from(config_string(config, "operation").is_some()),
            Vec::len,
        )
}

fn output_or_config<'a>(
    output: &'a Map<String, Value>,
    config: &'a Map<String, Value>,
    output_key: &str,
    config_key: &str,
) -> Option<&'a Value> {
    output.get(output_key).or_else(|| config.get(config_key))
}

fn output_value<'a>(output: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    output.get(key)
}

fn quoted(value: Option<&Value>) -> String {
    value.map_or_else(|| "\"unknown\"".to_owned(), diagnostic_value)
}

fn displayed(value: Option<&Value>) -> String {
    value.map_or_else(|| "unknown".to_owned(), diagnostic_value)
}

fn diagnostic_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value_to_string(value))
}

fn diagnostic_map(value: &Map<String, Value>) -> String {
    diagnostic_value(&Value::Object(value.clone()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn produces_action_specific_messages() {
        let cases = [
            (
                "action.file.copy",
                json!({"sourcePath": "a.txt", "destinationPath": "b.txt"}),
                json!({"bytes": 12}),
                "Copied file \"a.txt\" to \"b.txt\". Bytes copied: 12. Inputs: {\"destinationPath\":\"b.txt\",\"sourcePath\":\"a.txt\"}. Outputs: {\"bytes\":12}.",
            ),
            (
                "action.pixel.get",
                json!({"x": 10, "y": 20}),
                json!({"hex": "#AABBCC"}),
                "Read pixel at x=10, y=20. Color: \"#AABBCC\". Inputs: {\"x\":10,\"y\":20}. Outputs: {\"hex\":\"#AABBCC\"}.",
            ),
            (
                "action.value.convert",
                json!({"targetType": "number"}),
                json!({"value": 42}),
                "Converted value to \"number\". Result: 42. Inputs: {\"targetType\":\"number\"}. Outputs: {\"value\":42}.",
            ),
        ];

        for (action_type, config, output, expected) in cases {
            assert_eq!(
                action_completion_message(
                    action_type,
                    config.as_object().expect("config"),
                    output.as_object().expect("output")
                )
                .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn escapes_control_characters_in_displayed_values() {
        let message = action_completion_message(
            "action.value.convert",
            json!({"targetType": "string"}).as_object().expect("config"),
            json!({"value": "line\r\nnext"})
                .as_object()
                .expect("output"),
        )
        .expect("message");

        assert!(message.contains(r#""line\r\nnext""#));
        assert!(!message.contains('\r'));
        assert!(!message.contains('\n'));
    }

    #[test]
    fn notification_message_includes_complete_resolved_data() {
        let message = action_completion_message(
            "action.notification",
            json!({"title": "Ping results", "message": "Reply from 1.1.1.1: 18 ms"})
                .as_object()
                .expect("config"),
            json!({"displayed": true}).as_object().expect("output"),
        )
        .expect("message");

        assert!(message.contains(
            "Displayed notification \"Ping results\" with message \"Reply from 1.1.1.1: 18 ms\"."
        ));
        assert!(message.contains(
            "Inputs: {\"message\":\"Reply from 1.1.1.1: 18 ms\",\"title\":\"Ping results\"}."
        ));
        assert!(message.contains("Outputs: {\"displayed\":true}."));
        assert!(!message.contains("character message"));
    }
}
