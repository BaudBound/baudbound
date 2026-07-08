use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use rfd::{MessageButtons, MessageDialogResult, MessageLevel};
use serde_json::{Map, Value};

use super::config::{config_string, failed_error, required_string};

pub(super) fn run_message_box(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let title = required_string(request, "title")?;
    let message = required_string(request, "message")?;
    let variant = config_string(request, "type").unwrap_or_else(|| "info".to_owned());
    let buttons = config_string(request, "buttons").unwrap_or_else(|| "ok".to_owned());
    let result = rfd::MessageDialog::new()
        .set_title(&title)
        .set_description(&message)
        .set_level(message_box_level(&variant))
        .set_buttons(message_box_buttons(&buttons))
        .show();
    let button = message_box_result(result);

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("button".to_owned(), Value::String(button)),
            ("buttons".to_owned(), Value::String(buttons)),
            ("message".to_owned(), Value::String(message)),
            ("title".to_owned(), Value::String(title)),
            (
                "type".to_owned(),
                Value::String(normalize_message_box_type(&variant)),
            ),
        ]),
    })
}

pub(super) fn run_notification(
    request: &RuntimeActionRequest,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let title = required_string(request, "title")?;
    let message = required_string(request, "message")?;
    notify_rust::Notification::new()
        .summary(&title)
        .body(&message)
        .show()
        .map_err(|source| failed_error(request, format!("notification failed: {source}")))?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("message".to_owned(), Value::String(message)),
            ("shown".to_owned(), Value::Bool(true)),
            ("title".to_owned(), Value::String(title)),
        ]),
    })
}

pub(super) fn message_box_level(value: &str) -> MessageLevel {
    match value.trim() {
        "error" => MessageLevel::Error,
        "warning" => MessageLevel::Warning,
        _ => MessageLevel::Info,
    }
}

pub(super) fn message_box_buttons(value: &str) -> MessageButtons {
    match value.trim() {
        "ok_cancel" => MessageButtons::OkCancel,
        "yes_no" => MessageButtons::YesNo,
        "yes_no_cancel" => MessageButtons::YesNoCancel,
        _ => MessageButtons::Ok,
    }
}

pub(super) fn message_box_result(value: MessageDialogResult) -> String {
    match value {
        MessageDialogResult::Ok => "ok".to_owned(),
        MessageDialogResult::Cancel => "cancel".to_owned(),
        MessageDialogResult::Yes => "yes".to_owned(),
        MessageDialogResult::No => "no".to_owned(),
        MessageDialogResult::Custom(value) => value,
    }
}

pub(super) fn normalize_message_box_type(value: &str) -> String {
    match value.trim() {
        "error" | "warning" => value.trim().to_owned(),
        _ => "info".to_owned(),
    }
}
