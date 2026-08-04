use std::{collections::BTreeSet, sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeCancellationToken,
    RuntimeContext,
};
use baudbound_script::{is_user_identifier, parse_rgb_color};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::config::{config_string, failed_error, required_string};

const MAX_TITLE_CHARS: usize = 200;
const MAX_DESCRIPTION_CHARS: usize = 16_384;
const MAX_PLACEHOLDER_CHARS: usize = 512;
const MAX_DEFAULT_VALUE_CHARS: usize = 16_384;
const MAX_FORM_FIELD_COUNT: usize = 50;
const MAX_FORM_FIELD_KEY_CHARS: usize = 64;
const MAX_FORM_FIELD_LABEL_CHARS: usize = 200;
const MAX_FORM_FIELD_DESCRIPTION_CHARS: usize = 2_048;
const MAX_CHOICE_COUNT: usize = 100;
const MAX_CHOICE_LABEL_CHARS: usize = 512;
const MAX_CHOICE_VALUE_CHARS: usize = 512;
const MAX_TIMEOUT_SECONDS: f64 = 86_400.0;
const MAX_FORM_IMAGE_BYTES: usize = 8 * 1024 * 1024;

pub(crate) trait DesktopDialogProvider: Send + Sync {
    fn show_dialog(
        &self,
        request: DesktopDialogRequest,
        cancellation: &RuntimeCancellationToken,
        timeout: Option<Duration>,
    ) -> Result<DesktopDialogResponse, DesktopDialogError>;
}

#[derive(Debug, Default)]
pub(crate) struct UnavailableDesktopDialogProvider;

impl DesktopDialogProvider for UnavailableDesktopDialogProvider {
    fn show_dialog(
        &self,
        _request: DesktopDialogRequest,
        _cancellation: &RuntimeCancellationToken,
        _timeout: Option<Duration>,
    ) -> Result<DesktopDialogResponse, DesktopDialogError> {
        Err(DesktopDialogError::Unavailable)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopDialogRequest {
    pub(crate) requesting_script: String,
    pub(crate) timeout_at_unix_ms: Option<u64>,
    pub(crate) title: String,
    #[serde(flatten)]
    pub(crate) content: DesktopDialogContent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum DesktopDialogContent {
    MessageDialog {
        buttons: MessageDialogButtons,
        dialog_size: DesktopDialogSize,
        message: String,
        variant: MessageDialogVariant,
    },
    FormDialog {
        description: String,
        dialog_size: DesktopDialogSize,
        #[serde(flatten)]
        form: FormDialogContent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DesktopDialogSize {
    Small,
    Medium,
    Large,
}

impl DesktopDialogSize {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

impl DesktopDialogRequest {
    #[must_use]
    pub(crate) fn close_response(&self) -> Option<DesktopDialogResponse> {
        match &self.content {
            DesktopDialogContent::MessageDialog { buttons, .. } => match buttons {
                MessageDialogButtons::Ok => Some(DesktopDialogResponse::button("ok")),
                MessageDialogButtons::OkCancel
                | MessageDialogButtons::CancelConfirm
                | MessageDialogButtons::YesNoCancel => {
                    Some(DesktopDialogResponse::button("cancel"))
                }
                MessageDialogButtons::YesNo => None,
            },
            DesktopDialogContent::FormDialog { .. } => {
                Some(DesktopDialogResponse::button("cancel"))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MessageDialogButtons {
    Ok,
    OkCancel,
    CancelConfirm,
    YesNo,
    YesNoCancel,
}

impl MessageDialogButtons {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "ok" => Some(Self::Ok),
            "ok_cancel" => Some(Self::OkCancel),
            "cancel_confirm" => Some(Self::CancelConfirm),
            "yes_no" => Some(Self::YesNo),
            "yes_no_cancel" => Some(Self::YesNoCancel),
            _ => None,
        }
    }

    fn allows(self, button: &str) -> bool {
        match self {
            Self::Ok => button == "ok",
            Self::OkCancel => matches!(button, "ok" | "cancel"),
            Self::CancelConfirm => matches!(button, "cancel" | "confirm"),
            Self::YesNo => matches!(button, "yes" | "no"),
            Self::YesNoCancel => matches!(button, "yes" | "no" | "cancel"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::OkCancel => "ok_cancel",
            Self::CancelConfirm => "cancel_confirm",
            Self::YesNo => "yes_no",
            Self::YesNoCancel => "yes_no_cancel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MessageDialogVariant {
    Error,
    Info,
    Warning,
}

impl MessageDialogVariant {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "error" => Some(Self::Error),
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Info => "info",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormDialogContent {
    fields: Vec<FormDialogField>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum FormDialogField {
    Checkbox {
        default_checked: bool,
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    Information {
        accent_color: String,
        description: String,
        label: String,
    },
    SectionHeading {
        accent_color: String,
        description: String,
        label: String,
    },
    Divider {
        accent_color: String,
    },
    Image {
        data_url: String,
        description: String,
        image_fit: String,
        image_height: u16,
        label: String,
    },
    MultiChoice {
        choices: Vec<DialogChoice>,
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    Multiline {
        default_value: String,
        description: String,
        key: String,
        label: String,
        placeholder: String,
        required: bool,
    },
    Number {
        default_value: Option<f64>,
        description: String,
        key: String,
        label: String,
        placeholder: String,
        required: bool,
    },
    Password {
        description: String,
        key: String,
        label: String,
        placeholder: String,
        required: bool,
    },
    SingleChoice {
        choices: Vec<DialogChoice>,
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    Dropdown {
        choices: Vec<DialogChoice>,
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    Date {
        default_value: String,
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    Time {
        default_value: String,
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    Datetime {
        default_value: String,
        description: String,
        key: String,
        label: String,
        required: bool,
        timezone: String,
    },
    Color {
        default_value: String,
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    File {
        description: String,
        key: String,
        label: String,
        multiple: bool,
        required: bool,
    },
    Folder {
        description: String,
        key: String,
        label: String,
        required: bool,
    },
    Slider {
        default_value: f64,
        description: String,
        key: String,
        label: String,
        maximum: f64,
        minimum: f64,
        required: bool,
        step: f64,
    },
    Text {
        default_value: String,
        description: String,
        key: String,
        label: String,
        placeholder: String,
        required: bool,
    },
}

impl FormDialogField {
    fn key(&self) -> Option<&str> {
        match self {
            Self::Information { .. }
            | Self::SectionHeading { .. }
            | Self::Divider { .. }
            | Self::Image { .. } => None,
            Self::Checkbox { key, .. }
            | Self::Color { key, .. }
            | Self::Date { key, .. }
            | Self::Datetime { key, .. }
            | Self::Dropdown { key, .. }
            | Self::File { key, .. }
            | Self::Folder { key, .. }
            | Self::MultiChoice { key, .. }
            | Self::Multiline { key, .. }
            | Self::Number { key, .. }
            | Self::Password { key, .. }
            | Self::SingleChoice { key, .. }
            | Self::Slider { key, .. }
            | Self::Time { key, .. }
            | Self::Text { key, .. } => Some(key),
        }
    }

    fn is_password(&self) -> bool {
        matches!(self, Self::Password { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DialogChoice {
    pub(crate) display_value: String,
    pub(crate) key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DesktopDialogResponse {
    pub(crate) button: String,
    #[serde(default)]
    pub(crate) values: Map<String, Value>,
}

impl DesktopDialogResponse {
    #[must_use]
    pub(crate) fn button(button: impl Into<String>) -> Self {
        Self {
            button: button.into(),
            values: Map::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DesktopDialogError {
    #[error("runtime was cancelled")]
    Cancelled,
    #[error("interactive desktop dialogs are unavailable in this runner mode")]
    Unavailable,
    #[error("desktop dialog failed: {0}")]
    Failed(String),
}

pub(super) fn run_message_box(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    provider: &dyn DesktopDialogProvider,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    ensure_not_cancelled(context)?;
    let title = required_string(request, "title")?;
    let message = required_string(request, "message")?;
    validate_length(request, "title", &title, MAX_TITLE_CHARS)?;
    validate_length(request, "message", &message, MAX_DESCRIPTION_CHARS)?;
    let variant_value = config_string(request, "type").unwrap_or_else(|| "info".to_owned());
    let variant = MessageDialogVariant::parse(&variant_value).ok_or_else(|| {
        failed_error(
            request,
            format!("unsupported message dialog type {variant_value:?}"),
        )
    })?;
    let buttons_value = required_string(request, "buttons")?;
    let buttons = MessageDialogButtons::parse(&buttons_value).ok_or_else(|| {
        failed_error(
            request,
            format!("unsupported message dialog button set {buttons_value:?}"),
        )
    })?;
    let dialog_size_value = required_string(request, "dialogSize")?;
    let dialog_size = DesktopDialogSize::parse(&dialog_size_value).ok_or_else(|| {
        failed_error(
            request,
            format!("unsupported message dialog window size {dialog_size_value:?}"),
        )
    })?;
    let timeout = parse_timeout(request)?;
    let response = show_dialog(
        request,
        context,
        provider,
        DesktopDialogRequest {
            requesting_script: context.identity.script_id.clone(),
            timeout_at_unix_ms: None,
            title: title.clone(),
            content: DesktopDialogContent::MessageDialog {
                buttons,
                dialog_size,
                message: message.clone(),
                variant,
            },
        },
        timeout,
    )?;
    if !response.values.is_empty() {
        return Err(failed_error(
            request,
            "message dialog response contained unexpected dialog data",
        ));
    }
    if response.button != "timeout" && !buttons.allows(&response.button) {
        return Err(failed_error(
            request,
            format!(
                "message dialog returned button {:?} outside configured set {}",
                response.button,
                buttons.as_str()
            ),
        ));
    }

    Ok(RuntimeActionResult::new(Map::from_iter([
        ("button".to_owned(), Value::String(response.button)),
        (
            "buttons".to_owned(),
            Value::String(buttons.as_str().to_owned()),
        ),
        (
            "dialogSize".to_owned(),
            Value::String(dialog_size.as_str().to_owned()),
        ),
        ("message".to_owned(), Value::String(message)),
        ("title".to_owned(), Value::String(title)),
        (
            "type".to_owned(),
            Value::String(variant.as_str().to_owned()),
        ),
    ])))
}

pub(super) fn run_form_dialog(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    provider: &dyn DesktopDialogProvider,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    ensure_not_cancelled(context)?;
    let title = required_string(request, "title")?;
    validate_length(request, "title", &title, MAX_TITLE_CHARS)?;
    let description = config_string(request, "description").unwrap_or_default();
    validate_length(request, "description", &description, MAX_DESCRIPTION_CHARS)?;
    let dialog_size_value = required_string(request, "dialogSize")?;
    let dialog_size = DesktopDialogSize::parse(&dialog_size_value).ok_or_else(|| {
        failed_error(
            request,
            format!("unsupported form dialog window size {dialog_size_value:?}"),
        )
    })?;
    let form = FormDialogContent {
        fields: parse_form_fields(request, context)?,
    };
    let timeout = parse_timeout(request)?;
    let response = show_dialog(
        request,
        context,
        provider,
        DesktopDialogRequest {
            requesting_script: context.identity.script_id.clone(),
            timeout_at_unix_ms: None,
            title,
            content: DesktopDialogContent::FormDialog {
                description,
                dialog_size,
                form: form.clone(),
            },
        },
        timeout,
    )?;
    let normalized = validate_form_dialog_response(request, &form.fields, response)?;
    let submitted = normalized.button == "ok";
    let password_keys = form
        .fields
        .iter()
        .filter(|field| field.is_password())
        .filter_map(FormDialogField::key)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut result = RuntimeActionResult::new(Map::from_iter([
        ("values".to_owned(), Value::Object(normalized.values)),
        ("submitted".to_owned(), Value::Bool(submitted)),
        ("button".to_owned(), Value::String(normalized.button)),
    ]));
    if submitted {
        for key in password_keys {
            result = result.with_sensitive_output_path("values", [key]);
        }
    }
    Ok(result)
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

    Ok(RuntimeActionResult::new(Map::from_iter([
        ("message".to_owned(), Value::String(message)),
        ("shown".to_owned(), Value::Bool(true)),
        ("title".to_owned(), Value::String(title)),
    ])))
}

fn show_dialog(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    provider: &dyn DesktopDialogProvider,
    dialog: DesktopDialogRequest,
    timeout: Option<Duration>,
) -> Result<DesktopDialogResponse, RuntimeActionError> {
    provider
        .show_dialog(dialog, &context.cancellation, timeout)
        .map_err(|error| match error {
            DesktopDialogError::Cancelled => RuntimeActionError::Cancelled,
            DesktopDialogError::Unavailable => failed_error(
                request,
                "DESKTOP_DIALOG_UNAVAILABLE: this action requires the BaudBound desktop application",
            ),
            DesktopDialogError::Failed(message) => failed_error(
                request,
                format!("DESKTOP_DIALOG_FAILED: {message}"),
            ),
        })
}

fn ensure_not_cancelled(context: &RuntimeContext) -> Result<(), RuntimeActionError> {
    if context.cancellation.is_cancelled() {
        Err(RuntimeActionError::Cancelled)
    } else {
        Ok(())
    }
}

fn parse_form_fields(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<Vec<FormDialogField>, RuntimeActionError> {
    let value = request
        .config
        .get("fields")
        .ok_or_else(|| failed_error(request, "form dialog requires a fields list"))?;
    let entries = value
        .as_array()
        .ok_or_else(|| failed_error(request, "form dialog fields must be a list"))?;
    if entries.is_empty() {
        return Err(failed_error(
            request,
            "form dialog requires at least one component",
        ));
    }
    if entries.len() > MAX_FORM_FIELD_COUNT {
        return Err(failed_error(
            request,
            format!("form dialog supports at most {MAX_FORM_FIELD_COUNT} components"),
        ));
    }
    let mut keys = BTreeSet::new();
    let mut fields = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let object = entry.as_object().ok_or_else(|| {
            failed_error(
                request,
                format!("form component {} must be an object", index + 1),
            )
        })?;
        let field_type = object_string(request, object, "type", index)?;
        let description = object_string(request, object, "description", index)?;
        validate_length(
            request,
            "form component description",
            &description,
            MAX_FORM_FIELD_DESCRIPTION_CHARS,
        )?;
        let label = object_string(request, object, "label", index)?;
        validate_length(
            request,
            "form component label",
            &label,
            MAX_FORM_FIELD_LABEL_CHARS,
        )?;
        if field_type == "information" {
            ensure_exact_fields(
                request,
                object,
                index,
                &["type", "label", "description", "accentColor"],
            )?;
            if label.trim().is_empty() && description.trim().is_empty() {
                return Err(failed_error(
                    request,
                    format!(
                        "form component {} requires a label or description",
                        index + 1
                    ),
                ));
            }
            let accent_color = normalize_form_accent_color(
                request,
                &object_string(request, object, "accentColor", index)?,
                index,
            )?;
            fields.push(FormDialogField::Information {
                accent_color,
                description,
                label,
            });
            continue;
        }
        if field_type == "section_heading" {
            ensure_exact_fields(
                request,
                object,
                index,
                &["type", "label", "description", "accentColor"],
            )?;
            if label.trim().is_empty() && description.trim().is_empty() {
                return Err(failed_error(
                    request,
                    format!(
                        "form component {} requires a label or description",
                        index + 1
                    ),
                ));
            }
            fields.push(FormDialogField::SectionHeading {
                accent_color: normalize_form_accent_color(
                    request,
                    &object_string(request, object, "accentColor", index)?,
                    index,
                )?,
                description,
                label,
            });
            continue;
        }
        if field_type == "divider" {
            ensure_exact_fields(
                request,
                object,
                index,
                &["type", "label", "description", "accentColor"],
            )?;
            fields.push(FormDialogField::Divider {
                accent_color: normalize_form_accent_color(
                    request,
                    &object_string(request, object, "accentColor", index)?,
                    index,
                )?,
            });
            continue;
        }
        if field_type == "image" {
            ensure_exact_fields(
                request,
                object,
                index,
                &[
                    "type",
                    "label",
                    "description",
                    "assetPath",
                    "imageFit",
                    "imageHeight",
                ],
            )?;
            let asset_path = object_string(request, object, "assetPath", index)?;
            let image_fit = object_string(request, object, "imageFit", index)?;
            if !matches!(image_fit.as_str(), "contain" | "cover") {
                return Err(failed_error(
                    request,
                    format!("form component {} image fit is unsupported", index + 1),
                ));
            }
            let image_height =
                required_object_number(request, object.get("imageHeight"), index, "imageHeight")?;
            if image_height.fract() != 0.0 || !(80.0..=600.0).contains(&image_height) {
                return Err(failed_error(
                    request,
                    format!(
                        "form component {} image height must be an integer from 80 to 600",
                        index + 1
                    ),
                ));
            }
            let asset = super::package_assets::read_context_package_asset(context, &asset_path)
                .map_err(|source| {
                    failed_error(
                        request,
                        format!("failed to read form image asset {asset_path:?}: {source}"),
                    )
                })?;
            if !matches!(
                asset.media_type.as_str(),
                "image/png" | "image/jpeg" | "image/webp" | "image/gif" | "image/svg+xml"
            ) {
                return Err(failed_error(
                    request,
                    format!(
                        "form component {} asset must be a supported image",
                        index + 1
                    ),
                ));
            }
            if asset.bytes.len() > MAX_FORM_IMAGE_BYTES {
                return Err(failed_error(
                    request,
                    format!("form component {} image must not exceed 8 MiB", index + 1),
                ));
            }
            fields.push(FormDialogField::Image {
                data_url: format!(
                    "data:{};base64,{}",
                    asset.media_type,
                    BASE64_STANDARD.encode(asset.bytes)
                ),
                description,
                image_fit,
                image_height: image_height as u16,
                label,
            });
            continue;
        }

        let key = object_string(request, object, "key", index)?.to_owned();
        validate_form_field_key(request, &key, index, &mut keys)?;
        if label.trim().is_empty() {
            return Err(failed_error(
                request,
                format!("form component {} requires a label", index + 1),
            ));
        }
        let required = object_bool(request, object, "required", index)?;
        let common = || (description.clone(), key.clone(), label.clone(), required);
        let field = match field_type.as_str() {
            "checkbox" => {
                ensure_exact_fields(
                    request,
                    object,
                    index,
                    &[
                        "type",
                        "key",
                        "label",
                        "description",
                        "required",
                        "defaultChecked",
                    ],
                )?;
                let (description, key, label, required) = common();
                FormDialogField::Checkbox {
                    default_checked: object_bool(request, object, "defaultChecked", index)?,
                    description,
                    key,
                    label,
                    required,
                }
            }
            "single_choice" | "multi_choice" | "dropdown" => {
                ensure_exact_fields(
                    request,
                    object,
                    index,
                    &["type", "key", "label", "description", "required", "choices"],
                )?;
                let choices = parse_choices(request, object.get("choices"), index)?;
                let (description, key, label, required) = common();
                if field_type == "single_choice" {
                    FormDialogField::SingleChoice {
                        choices,
                        description,
                        key,
                        label,
                        required,
                    }
                } else if field_type == "multi_choice" {
                    FormDialogField::MultiChoice {
                        choices,
                        description,
                        key,
                        label,
                        required,
                    }
                } else {
                    FormDialogField::Dropdown {
                        choices,
                        description,
                        key,
                        label,
                        required,
                    }
                }
            }
            "date" | "time" | "datetime" | "color" => {
                let expected = if field_type == "datetime" {
                    vec![
                        "type",
                        "key",
                        "label",
                        "description",
                        "required",
                        "defaultValue",
                        "timezone",
                    ]
                } else {
                    vec![
                        "type",
                        "key",
                        "label",
                        "description",
                        "required",
                        "defaultValue",
                    ]
                };
                ensure_exact_fields(request, object, index, &expected)?;
                let timezone = if field_type == "datetime" {
                    let timezone = object_string(request, object, "timezone", index)?;
                    validate_timezone_name(request, &timezone, index)?;
                    Some(timezone)
                } else {
                    None
                };
                let default_value = if field_type == "color" {
                    object_string(request, object, "defaultValue", index)?
                } else {
                    resolve_temporal_default(
                        request,
                        &field_type,
                        object.get("defaultValue"),
                        timezone.as_deref(),
                        index,
                    )?
                };
                let (description, key, label, required) = common();
                match field_type.as_str() {
                    "date" => FormDialogField::Date {
                        default_value,
                        description,
                        key,
                        label,
                        required,
                    },
                    "time" => FormDialogField::Time {
                        default_value,
                        description,
                        key,
                        label,
                        required,
                    },
                    "datetime" => {
                        let timezone = timezone.ok_or_else(|| {
                            failed_error(
                                request,
                                format!("form component {} requires timezone", index + 1),
                            )
                        })?;
                        FormDialogField::Datetime {
                            default_value,
                            description,
                            key,
                            label,
                            required,
                            timezone,
                        }
                    }
                    "color" => FormDialogField::Color {
                        default_value: normalize_form_accent_color(request, &default_value, index)?,
                        description,
                        key,
                        label,
                        required,
                    },
                    _ => unreachable!(),
                }
            }
            "file" => {
                ensure_exact_fields(
                    request,
                    object,
                    index,
                    &[
                        "type",
                        "key",
                        "label",
                        "description",
                        "required",
                        "multiple",
                    ],
                )?;
                let (description, key, label, required) = common();
                FormDialogField::File {
                    description,
                    key,
                    label,
                    multiple: object_bool(request, object, "multiple", index)?,
                    required,
                }
            }
            "folder" => {
                ensure_exact_fields(
                    request,
                    object,
                    index,
                    &["type", "key", "label", "description", "required"],
                )?;
                let (description, key, label, required) = common();
                FormDialogField::Folder {
                    description,
                    key,
                    label,
                    required,
                }
            }
            "slider" => {
                ensure_exact_fields(
                    request,
                    object,
                    index,
                    &[
                        "type",
                        "key",
                        "label",
                        "description",
                        "required",
                        "defaultValue",
                        "minimum",
                        "maximum",
                        "step",
                    ],
                )?;
                let minimum =
                    required_object_number(request, object.get("minimum"), index, "minimum")?;
                let maximum =
                    required_object_number(request, object.get("maximum"), index, "maximum")?;
                let step = required_object_number(request, object.get("step"), index, "step")?;
                let default_value = required_object_number(
                    request,
                    object.get("defaultValue"),
                    index,
                    "defaultValue",
                )?;
                if maximum <= minimum {
                    return Err(failed_error(
                        request,
                        format!(
                            "form component {} slider maximum must be greater than minimum",
                            index + 1
                        ),
                    ));
                }
                if step <= 0.0 {
                    return Err(failed_error(
                        request,
                        format!(
                            "form component {} slider step must be greater than zero",
                            index + 1
                        ),
                    ));
                }
                if !(minimum..=maximum).contains(&default_value) {
                    return Err(failed_error(
                        request,
                        format!(
                            "form component {} slider default value must be within its range",
                            index + 1
                        ),
                    ));
                }
                let (description, key, label, required) = common();
                FormDialogField::Slider {
                    default_value,
                    description,
                    key,
                    label,
                    maximum,
                    minimum,
                    required,
                    step,
                }
            }
            "password" => {
                ensure_exact_fields(
                    request,
                    object,
                    index,
                    &[
                        "type",
                        "key",
                        "label",
                        "description",
                        "required",
                        "placeholder",
                    ],
                )?;
                let placeholder = object_string(request, object, "placeholder", index)?;
                validate_length(
                    request,
                    "form placeholder",
                    &placeholder,
                    MAX_PLACEHOLDER_CHARS,
                )?;
                let (description, key, label, required) = common();
                FormDialogField::Password {
                    description,
                    key,
                    label,
                    placeholder,
                    required,
                }
            }
            "text" | "multiline" | "number" => {
                ensure_exact_fields(
                    request,
                    object,
                    index,
                    &[
                        "type",
                        "key",
                        "label",
                        "description",
                        "required",
                        "placeholder",
                        "defaultValue",
                    ],
                )?;
                let placeholder = object_string(request, object, "placeholder", index)?;
                validate_length(
                    request,
                    "form placeholder",
                    &placeholder,
                    MAX_PLACEHOLDER_CHARS,
                )?;
                let (description, key, label, required) = common();
                if field_type == "number" {
                    FormDialogField::Number {
                        default_value: optional_number(request, object.get("defaultValue"), index)?,
                        description,
                        key,
                        label,
                        placeholder,
                        required,
                    }
                } else {
                    let default_value = object_string(request, object, "defaultValue", index)?;
                    validate_length(
                        request,
                        "form default value",
                        &default_value,
                        MAX_DEFAULT_VALUE_CHARS,
                    )?;
                    if field_type == "text" {
                        FormDialogField::Text {
                            default_value,
                            description,
                            key,
                            label,
                            placeholder,
                            required,
                        }
                    } else {
                        FormDialogField::Multiline {
                            default_value,
                            description,
                            key,
                            label,
                            placeholder,
                            required,
                        }
                    }
                }
            }
            _ => {
                return Err(failed_error(
                    request,
                    format!(
                        "form component {} has unsupported type {field_type:?}",
                        index + 1
                    ),
                ));
            }
        };
        fields.push(field);
    }
    Ok(fields)
}

fn normalize_form_accent_color(
    request: &RuntimeActionRequest,
    value: &str,
    index: usize,
) -> Result<String, RuntimeActionError> {
    let color =
        parse_rgb_color(&Value::String(value.to_owned()), "accent color").map_err(|message| {
            failed_error(request, format!("form component {} {message}", index + 1))
        })?;
    Ok(format!(
        "#{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue
    ))
}

fn parse_choices(
    request: &RuntimeActionRequest,
    value: Option<&Value>,
    field_index: usize,
) -> Result<Vec<DialogChoice>, RuntimeActionError> {
    let entries = value.and_then(Value::as_array).ok_or_else(|| {
        failed_error(
            request,
            format!("form component {} choices must be a list", field_index + 1),
        )
    })?;
    if entries.is_empty() || entries.len() > MAX_CHOICE_COUNT {
        return Err(failed_error(
            request,
            format!(
                "form component {} choices must contain between 1 and {MAX_CHOICE_COUNT} entries",
                field_index + 1
            ),
        ));
    }
    let mut display_values = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut choices = Vec::with_capacity(entries.len());
    for (choice_index, entry) in entries.iter().enumerate() {
        let object = entry.as_object().ok_or_else(|| {
            failed_error(
                request,
                format!(
                    "form component {}, choice {} must be an object",
                    field_index + 1,
                    choice_index + 1
                ),
            )
        })?;
        if object.len() != 2 || !object.contains_key("key") || !object.contains_key("displayValue")
        {
            return Err(failed_error(
                request,
                format!(
                    "form component {}, choice {} may contain only key and displayValue",
                    field_index + 1,
                    choice_index + 1
                ),
            ));
        }
        let key = object
            .get("key")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                failed_error(
                    request,
                    format!(
                        "form component {}, choice {} requires a non-empty key",
                        field_index + 1,
                        choice_index + 1
                    ),
                )
            })?
            .to_owned();
        let display_value = object
            .get("displayValue")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                failed_error(
                    request,
                    format!(
                        "form component {}, choice {} requires a non-empty displayed value",
                        field_index + 1,
                        choice_index + 1
                    ),
                )
            })?
            .to_owned();
        validate_length(request, "choice key", &key, MAX_CHOICE_VALUE_CHARS)?;
        validate_form_key_characters(
            request,
            &key,
            &format!(
                "form component {}, choice {} key",
                field_index + 1,
                choice_index + 1
            ),
        )?;
        validate_length(
            request,
            "choice displayed value",
            &display_value,
            MAX_CHOICE_LABEL_CHARS,
        )?;
        if !keys.insert(key.clone()) {
            return Err(failed_error(
                request,
                format!("choice keys must be unique; duplicate {key:?}"),
            ));
        }
        if !display_values.insert(display_value.clone()) {
            return Err(failed_error(
                request,
                format!("choice displayed values must be unique; duplicate {display_value:?}"),
            ));
        }
        choices.push(DialogChoice { display_value, key });
    }
    Ok(choices)
}

fn ensure_exact_fields(
    request: &RuntimeActionRequest,
    object: &Map<String, Value>,
    index: usize,
    expected: &[&str],
) -> Result<(), RuntimeActionError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(failed_error(
            request,
            format!(
                "form component {} fields do not match its configured type",
                index + 1
            ),
        ))
    }
}

fn object_string(
    request: &RuntimeActionRequest,
    object: &Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<String, RuntimeActionError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            failed_error(
                request,
                format!("form component {} {key} must be text", index + 1),
            )
        })
}

fn object_bool(
    request: &RuntimeActionRequest,
    object: &Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<bool, RuntimeActionError> {
    object.get(key).and_then(Value::as_bool).ok_or_else(|| {
        failed_error(
            request,
            format!("form component {} {key} must be boolean", index + 1),
        )
    })
}

fn required_object_number(
    request: &RuntimeActionRequest,
    value: Option<&Value>,
    index: usize,
    name: &str,
) -> Result<f64, RuntimeActionError> {
    value
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        })
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            failed_error(
                request,
                format!(
                    "form component {} {name} must be a finite number",
                    index + 1
                ),
            )
        })
}

fn validate_temporal_default(
    request: &RuntimeActionRequest,
    field_type: &str,
    value: &str,
    index: usize,
) -> Result<(), RuntimeActionError> {
    if value.is_empty() || field_type == "color" {
        return Ok(());
    }
    let valid = match field_type {
        "date" => NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok(),
        "time" => NaiveTime::parse_from_str(value, "%H:%M:%S")
            .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M"))
            .is_ok(),
        "datetime" => NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))
            .is_ok(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(failed_error(
            request,
            format!(
                "form component {} default value is not a valid {field_type}",
                index + 1
            ),
        ))
    }
}

fn resolve_temporal_default(
    request: &RuntimeActionRequest,
    field_type: &str,
    value: Option<&Value>,
    timezone: Option<&str>,
    index: usize,
) -> Result<String, RuntimeActionError> {
    let Some(value) = value else {
        return Err(failed_error(
            request,
            format!("form component {} requires defaultValue", index + 1),
        ));
    };
    if let Some(text) = value.as_str() {
        validate_temporal_default(request, field_type, text, index)?;
        return Ok(text.to_owned());
    }

    let Some(object) = value.as_object() else {
        return Err(invalid_temporal_default_type(request, index));
    };
    if object.get("type").and_then(Value::as_str) != Some("datetime") {
        return Err(invalid_temporal_default_type(request, index));
    }
    let Some(iso) = object.get("value").and_then(Value::as_str) else {
        return Err(invalid_temporal_default_type(request, index));
    };
    let datetime = DateTime::parse_from_rfc3339(iso).map_err(|_| {
        failed_error(
            request,
            format!(
                "form component {} default datetime has an invalid RFC 3339 value",
                index + 1
            ),
        )
    })?;

    let formatted = match field_type {
        "date" => datetime
            .with_timezone(&Local)
            .format("%Y-%m-%d")
            .to_string(),
        "time" => datetime
            .with_timezone(&Local)
            .format("%H:%M:%S")
            .to_string(),
        "datetime" => format_datetime_in_timezone(request, datetime, timezone, index)?,
        _ => return Err(invalid_temporal_default_type(request, index)),
    };
    validate_temporal_default(request, field_type, &formatted, index)?;
    Ok(formatted)
}

fn format_datetime_in_timezone(
    request: &RuntimeActionRequest,
    datetime: DateTime<chrono::FixedOffset>,
    timezone: Option<&str>,
    index: usize,
) -> Result<String, RuntimeActionError> {
    let timezone = timezone.ok_or_else(|| {
        failed_error(
            request,
            format!("form component {} requires timezone", index + 1),
        )
    })?;
    let formatted = match timezone {
        "__local__" => datetime
            .with_timezone(&Local)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        "UTC" => datetime
            .with_timezone(&Utc)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        value => datetime
            .with_timezone(&value.parse::<Tz>().map_err(|_| {
                failed_error(
                    request,
                    format!("form component {} timezone is invalid", index + 1),
                )
            })?)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
    };
    Ok(formatted)
}

fn invalid_temporal_default_type(
    request: &RuntimeActionRequest,
    index: usize,
) -> RuntimeActionError {
    failed_error(
        request,
        format!(
            "form component {} defaultValue must be text or a datetime value",
            index + 1
        ),
    )
}

fn validate_timezone_name(
    request: &RuntimeActionRequest,
    value: &str,
    index: usize,
) -> Result<(), RuntimeActionError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && (value == "__local__" || value.parse::<Tz>().is_ok());
    if valid {
        Ok(())
    } else {
        Err(failed_error(
            request,
            format!("form component {} timezone is invalid", index + 1),
        ))
    }
}

fn optional_number(
    request: &RuntimeActionRequest,
    value: Option<&Value>,
    index: usize,
) -> Result<Option<f64>, RuntimeActionError> {
    let Some(value) = value else {
        return Err(failed_error(
            request,
            format!("form component {} requires defaultValue", index + 1),
        ));
    };
    if value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .map(Some)
        .ok_or_else(|| {
            failed_error(
                request,
                format!(
                    "form component {} defaultValue must be a finite number",
                    index + 1
                ),
            )
        })
}

fn validate_form_field_key(
    request: &RuntimeActionRequest,
    key: &str,
    index: usize,
    keys: &mut BTreeSet<String>,
) -> Result<(), RuntimeActionError> {
    validate_length(request, "form component key", key, MAX_FORM_FIELD_KEY_CHARS)?;
    validate_form_key_characters(request, key, &format!("form component {} key", index + 1))?;
    if !keys.insert(key.to_owned()) {
        return Err(failed_error(
            request,
            format!("form component keys must be unique; duplicate {key:?}"),
        ));
    }
    Ok(())
}

fn validate_form_key_characters(
    request: &RuntimeActionRequest,
    key: &str,
    label: &str,
) -> Result<(), RuntimeActionError> {
    if is_user_identifier(key) {
        return Ok(());
    }
    Err(failed_error(
        request,
        format!("{label} may contain only letters A-Z, a-z, numbers 0-9, hyphens, and underscores"),
    ))
}

fn parse_timeout(request: &RuntimeActionRequest) -> Result<Option<Duration>, RuntimeActionError> {
    let Some(value) = request.config.get("timeoutSeconds") else {
        return Ok(None);
    };
    if value.as_str().is_some_and(|value| value.trim().is_empty()) {
        return Ok(None);
    }
    let seconds = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0 && *seconds <= MAX_TIMEOUT_SECONDS)
        .ok_or_else(|| {
            failed_error(
                request,
                format!("timeoutSeconds must be greater than 0 and at most {MAX_TIMEOUT_SECONDS}"),
            )
        })?;
    Duration::try_from_secs_f64(seconds)
        .map(Some)
        .map_err(|source| failed_error(request, format!("invalid timeoutSeconds: {source}")))
}

fn validate_form_dialog_response(
    request: &RuntimeActionRequest,
    fields: &[FormDialogField],
    mut response: DesktopDialogResponse,
) -> Result<DesktopDialogResponse, RuntimeActionError> {
    if !matches!(response.button.as_str(), "ok" | "cancel" | "timeout") {
        return Err(failed_error(
            request,
            format!(
                "form dialog returned unsupported button {:?}",
                response.button
            ),
        ));
    }
    if response.button != "ok" {
        if !response.values.is_empty() {
            return Err(failed_error(
                request,
                "cancelled or timed-out form dialog response contained unexpected data",
            ));
        }
        return Ok(response);
    }
    let mut normalized = Map::new();
    for field in fields {
        validate_form_dialog_response_field(request, field, &mut response.values, &mut normalized)?;
    }
    if let Some(unknown) = response.values.keys().next() {
        return Err(failed_error(
            request,
            format!("form dialog response contains unknown field {unknown:?}"),
        ));
    }
    response.values = normalized;
    Ok(response)
}

fn validate_form_dialog_response_field(
    request: &RuntimeActionRequest,
    field: &FormDialogField,
    submitted: &mut Map<String, Value>,
    normalized: &mut Map<String, Value>,
) -> Result<(), RuntimeActionError> {
    let Some(key) = field.key() else {
        return Ok(());
    };
    let value = submitted.remove(key);
    match field {
        FormDialogField::Text { required, .. }
        | FormDialogField::Password { required, .. }
        | FormDialogField::Multiline { required, .. }
        | FormDialogField::Date { required, .. }
        | FormDialogField::Time { required, .. }
        | FormDialogField::Datetime { required, .. }
        | FormDialogField::Color { required, .. }
        | FormDialogField::File {
            required,
            multiple: false,
            ..
        }
        | FormDialogField::Folder { required, .. } => {
            let text = value
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    failed_error(
                        request,
                        format!("form dialog response field {key:?} must be text"),
                    )
                })?;
            if *required && text.is_empty() {
                return Err(failed_error(
                    request,
                    format!("form dialog response field {key:?} is required"),
                ));
            }
            validate_length(
                request,
                "form dialog response",
                &text,
                MAX_DEFAULT_VALUE_CHARS,
            )?;
            match field {
                FormDialogField::Date { .. } if !text.is_empty() => {
                    NaiveDate::parse_from_str(&text, "%Y-%m-%d").map_err(|_| {
                        failed_error(
                            request,
                            format!("form dialog response field {key:?} must be a valid date"),
                        )
                    })?;
                }
                FormDialogField::Time { .. } if !text.is_empty() => {
                    NaiveTime::parse_from_str(&text, "%H:%M:%S")
                        .or_else(|_| NaiveTime::parse_from_str(&text, "%H:%M"))
                        .map_err(|_| {
                            failed_error(
                                request,
                                format!("form dialog response field {key:?} must be a valid time"),
                            )
                        })?;
                }
                FormDialogField::Datetime { .. } if !text.is_empty() => {
                    DateTime::parse_from_rfc3339(&text).map_err(|_| {
                        failed_error(
                            request,
                            format!(
                                "form dialog response field {key:?} must be an ISO 8601 timestamp"
                            ),
                        )
                    })?;
                }
                FormDialogField::Color { .. } => {
                    let color = normalize_form_accent_color(request, &text, 0)?;
                    normalized.insert(key.to_owned(), Value::String(color));
                    return Ok(());
                }
                _ => {}
            }
            normalized.insert(key.to_owned(), Value::String(text));
        }
        FormDialogField::Number { required, .. } | FormDialogField::Slider { required, .. } => {
            match value {
                Some(value) => {
                    let number = value
                        .as_f64()
                        .filter(|number| number.is_finite())
                        .ok_or_else(|| {
                            failed_error(
                                request,
                                format!(
                                    "form dialog response field {key:?} must be a finite number"
                                ),
                            )
                        })?;
                    if let FormDialogField::Slider {
                        maximum,
                        minimum,
                        step,
                        ..
                    } = field
                    {
                        if !(*minimum..=*maximum).contains(&number) {
                            return Err(failed_error(
                                request,
                                format!(
                                    "form dialog response field {key:?} is outside its slider range"
                                ),
                            ));
                        }
                        let steps = (number - minimum) / step;
                        if (steps - steps.round()).abs() > f64::EPSILON * steps.abs().max(1.0) * 8.0
                        {
                            return Err(failed_error(
                                request,
                                format!(
                                    "form dialog response field {key:?} does not align with its slider step"
                                ),
                            ));
                        }
                    }
                    let number = serde_json::Number::from_f64(number).ok_or_else(|| {
                        failed_error(
                            request,
                            format!(
                                "form dialog response field {key:?} is outside the numeric range"
                            ),
                        )
                    })?;
                    normalized.insert(key.to_owned(), Value::Number(number));
                }
                None if *required => {
                    return Err(failed_error(
                        request,
                        format!("form dialog response field {key:?} is required"),
                    ));
                }
                None => {}
            }
        }
        FormDialogField::Checkbox { required, .. } => {
            let checked = value.and_then(|value| value.as_bool()).ok_or_else(|| {
                failed_error(
                    request,
                    format!("form dialog response field {key:?} must be boolean"),
                )
            })?;
            if *required && !checked {
                return Err(failed_error(
                    request,
                    format!("form dialog response field {key:?} must be checked"),
                ));
            }
            normalized.insert(key.to_owned(), Value::Bool(checked));
        }
        FormDialogField::SingleChoice {
            choices, required, ..
        }
        | FormDialogField::Dropdown {
            choices, required, ..
        } => {
            let selected = value
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    failed_error(
                        request,
                        format!("form dialog response field {key:?} must be text"),
                    )
                })?;
            if *required && selected.is_empty() {
                return Err(failed_error(
                    request,
                    format!("form dialog response field {key:?} requires a selection"),
                ));
            }
            if !selected.is_empty() && !choices.iter().any(|choice| choice.key == selected) {
                return Err(failed_error(
                    request,
                    format!(
                        "form dialog response field {key:?} contains unknown choice {selected:?}"
                    ),
                ));
            }
            normalized.insert(key.to_owned(), Value::String(selected));
        }
        FormDialogField::MultiChoice {
            choices, required, ..
        } => {
            let selected = value
                .and_then(|value| value.as_array().cloned())
                .ok_or_else(|| {
                    failed_error(
                        request,
                        format!("form dialog response field {key:?} must be a list"),
                    )
                })?;
            let selected = selected
                .into_iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        failed_error(
                            request,
                            format!("form dialog response field {key:?} choices must be text"),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let selected_set = selected.iter().map(String::as_str).collect::<BTreeSet<_>>();
            if selected_set.len() != selected.len() {
                return Err(failed_error(
                    request,
                    format!("form dialog response field {key:?} contains duplicate choices"),
                ));
            }
            if *required && selected.is_empty() {
                return Err(failed_error(
                    request,
                    format!("form dialog response field {key:?} requires a selection"),
                ));
            }
            if let Some(unknown) = selected_set.iter().find(|selected| {
                !choices
                    .iter()
                    .any(|choice| choice.key.as_str() == **selected)
            }) {
                return Err(failed_error(
                    request,
                    format!(
                        "form dialog response field {key:?} contains unknown choice {unknown:?}"
                    ),
                ));
            }
            normalized.insert(
                key.to_owned(),
                Value::Array(
                    choices
                        .iter()
                        .filter(|choice| selected_set.contains(choice.key.as_str()))
                        .map(|choice| Value::String(choice.key.clone()))
                        .collect(),
                ),
            );
        }
        FormDialogField::File {
            multiple: true,
            required,
            ..
        } => {
            let paths = value
                .and_then(|value| value.as_array().cloned())
                .ok_or_else(|| {
                    failed_error(
                        request,
                        format!("form dialog response field {key:?} must be a list"),
                    )
                })?;
            if *required && paths.is_empty() {
                return Err(failed_error(
                    request,
                    format!("form dialog response field {key:?} requires at least one file"),
                ));
            }
            let paths = paths
                .into_iter()
                .map(|path| {
                    let path = path.as_str().ok_or_else(|| {
                        failed_error(
                            request,
                            format!(
                                "form dialog response field {key:?} must contain only file paths"
                            ),
                        )
                    })?;
                    validate_length(
                        request,
                        "form dialog file path",
                        path,
                        MAX_DEFAULT_VALUE_CHARS,
                    )?;
                    Ok(Value::String(path.to_owned()))
                })
                .collect::<Result<Vec<_>, RuntimeActionError>>()?;
            normalized.insert(key.to_owned(), Value::Array(paths));
        }
        FormDialogField::Information { .. }
        | FormDialogField::SectionHeading { .. }
        | FormDialogField::Divider { .. }
        | FormDialogField::Image { .. } => {}
    }
    Ok(())
}

fn validate_length(
    request: &RuntimeActionRequest,
    field: &str,
    value: &str,
    maximum: usize,
) -> Result<(), RuntimeActionError> {
    if value.chars().count() <= maximum {
        Ok(())
    } else {
        Err(failed_error(
            request,
            format!("{field} exceeds the {maximum} character limit"),
        ))
    }
}

pub(crate) type SharedDesktopDialogProvider = Arc<dyn DesktopDialogProvider>;
