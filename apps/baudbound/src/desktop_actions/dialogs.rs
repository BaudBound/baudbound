use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use serde_json::{Map, Value};

use super::config::{config_string, failed_error, required_string};

#[cfg(windows)]
use std::{
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{FALSE, HWND, LPARAM, TRUE},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        EndDialog, EnumThreadWindows, GetClassNameW, IDCANCEL, IDNO, IDOK, IDYES, MB_ICONERROR,
        MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_OKCANCEL, MB_YESNO, MB_YESNOCANCEL,
        MessageBoxW,
    },
};

#[cfg(windows)]
const DIALOG_CANCELLATION_INTERVAL: Duration = Duration::from_millis(25);

#[cfg(windows)]
pub(super) fn run_message_box(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    if context.cancellation.is_cancelled() {
        return Err(RuntimeActionError::Cancelled);
    }
    let title = required_string(request, "title")?;
    let message = required_string(request, "message")?;
    let variant = config_string(request, "type").unwrap_or_else(|| "info".to_owned());
    let buttons = config_string(request, "buttons").unwrap_or_else(|| "ok".to_owned());
    let title_utf16 = wide_string(&title);
    let message_utf16 = wide_string(&message);
    let dialog_thread_id = unsafe { GetCurrentThreadId() };
    let completed = Arc::new(AtomicBool::new(false));
    let watcher_completed = Arc::clone(&completed);
    let cancellation = context.cancellation.clone();
    let watcher = thread::Builder::new()
        .name("baudbound-message-box-cancellation".to_owned())
        .spawn(move || {
            while !watcher_completed.load(Ordering::Acquire) {
                if cancellation.wait_for(DIALOG_CANCELLATION_INTERVAL) {
                    while !watcher_completed.load(Ordering::Acquire) {
                        unsafe {
                            EnumThreadWindows(dialog_thread_id, Some(close_message_box_window), 0);
                        }
                        thread::sleep(DIALOG_CANCELLATION_INTERVAL);
                    }
                    return;
                }
            }
        })
        .map_err(|source| {
            failed_error(
                request,
                format!("failed to start message box cancellation watcher: {source}"),
            )
        })?;
    let result = unsafe {
        MessageBoxW(
            ptr::null_mut(),
            message_utf16.as_ptr(),
            title_utf16.as_ptr(),
            message_box_flags(&variant, &buttons),
        )
    };
    completed.store(true, Ordering::Release);
    watcher
        .join()
        .map_err(|_| failed_error(request, "message box cancellation watcher panicked"))?;
    if context.cancellation.is_cancelled() {
        return Err(RuntimeActionError::Cancelled);
    }
    let button = message_box_result(result).ok_or_else(|| {
        failed_error(
            request,
            format!("message box returned unexpected native result {result}"),
        )
    })?;

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

#[cfg(not(windows))]
pub(super) fn run_message_box(
    request: &RuntimeActionRequest,
    _context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    Err(RuntimeActionError::Unsupported(request.action_type.clone()))
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

#[cfg(windows)]
fn message_box_flags(variant: &str, buttons: &str) -> u32 {
    message_box_level(variant) | message_box_buttons(buttons)
}

#[cfg(windows)]
pub(super) fn message_box_level(value: &str) -> u32 {
    match value.trim() {
        "error" => MB_ICONERROR,
        "warning" => MB_ICONWARNING,
        _ => MB_ICONINFORMATION,
    }
}

#[cfg(windows)]
pub(super) fn message_box_buttons(value: &str) -> u32 {
    match value.trim() {
        "ok_cancel" => MB_OKCANCEL,
        "yes_no" => MB_YESNO,
        "yes_no_cancel" => MB_YESNOCANCEL,
        _ => MB_OK,
    }
}

#[cfg(windows)]
pub(super) fn message_box_result(value: i32) -> Option<String> {
    match value {
        IDOK => Some("ok".to_owned()),
        IDCANCEL => Some("cancel".to_owned()),
        IDYES => Some("yes".to_owned()),
        IDNO => Some("no".to_owned()),
        _ => None,
    }
}

pub(super) fn normalize_message_box_type(value: &str) -> String {
    match value.trim() {
        "error" | "warning" => value.trim().to_owned(),
        _ => "info".to_owned(),
    }
}

#[cfg(windows)]
fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
unsafe extern "system" fn close_message_box_window(window: HWND, _parameter: LPARAM) -> i32 {
    const DIALOG_CLASS: &[u16] = &[
        b'#' as u16,
        b'3' as u16,
        b'2' as u16,
        b'7' as u16,
        b'7' as u16,
    ];
    let mut class_name = [0_u16; 32];
    let length = unsafe { GetClassNameW(window, class_name.as_mut_ptr(), class_name.len() as i32) };
    if length > 0 && class_name[..length as usize] == *DIALOG_CLASS {
        unsafe {
            EndDialog(window, IDCANCEL as isize);
        }
        FALSE
    } else {
        TRUE
    }
}
