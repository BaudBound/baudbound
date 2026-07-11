use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;

use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest};
use serde_json::{Map, Number, Value};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM},
    Graphics::Gdi::{GetDC, GetPixel, ReleaseDC},
    System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    },
    UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
    },
};

use super::{config::failed_error, screen::pixel_color_map};

pub(in crate::desktop_actions) mod process;

pub(super) fn pixel_color(
    request: &RuntimeActionRequest,
    x: u32,
    y: u32,
) -> Result<Map<String, Value>, RuntimeActionError> {
    unsafe {
        let device_context = GetDC(std::ptr::null_mut());
        if device_context.is_null() {
            return Err(failed_error(request, "failed to get screen device context"));
        }
        let pixel = GetPixel(device_context, x as i32, y as i32);
        ReleaseDC(std::ptr::null_mut(), device_context);
        if pixel == u32::MAX {
            return Err(failed_error(request, "failed to read screen pixel color"));
        }

        let red = (pixel & 0x0000_00ff) as u8;
        let green = ((pixel & 0x0000_ff00) >> 8) as u8;
        let blue = ((pixel & 0x00ff_0000) >> 16) as u8;
        Ok(pixel_color_map(x, y, red, green, blue, 255))
    }
}

pub(super) fn active_window(
    request: &RuntimeActionRequest,
) -> Result<Map<String, Value>, RuntimeActionError> {
    unsafe {
        let handle = GetForegroundWindow();
        if handle.is_null() {
            return Err(failed_error(
                request,
                "no active foreground window was found",
            ));
        }
        Ok(window_output(handle))
    }
}

pub(super) fn window_focus(
    request: &RuntimeActionRequest,
    match_mode: &str,
    target: &str,
) -> Result<(), RuntimeActionError> {
    let Some(handle) = find_window(match_mode, target) else {
        return Err(failed_error(
            request,
            format!("no matching window found for {match_mode}={target:?}"),
        ));
    };

    unsafe {
        if SetForegroundWindow(handle) == 0 {
            return Err(failed_error(request, "failed to focus matching window"));
        }
    }
    Ok(())
}

fn window_output(handle: HWND) -> Map<String, Value> {
    let process_id = window_process_id(handle);
    let executable_path = process_executable_path(process_id);
    let process_name = executable_path
        .as_deref()
        .and_then(process_name_from_path)
        .unwrap_or_default();
    Map::from_iter([
        ("title".to_owned(), Value::String(window_title(handle))),
        (
            "process_name".to_owned(),
            Value::String(process_name.to_owned()),
        ),
        (
            "process_id".to_owned(),
            Value::Number(Number::from(process_id)),
        ),
        (
            "executable_path".to_owned(),
            executable_path.map_or(Value::Null, Value::String),
        ),
    ])
}

fn find_window(match_mode: &str, target: &str) -> Option<HWND> {
    let mut state = WindowSearch {
        match_mode,
        target,
        result: None,
    };
    unsafe {
        let state_pointer = (&mut state as *mut WindowSearch).cast::<()>() as LPARAM;
        EnumWindows(Some(enum_windows_callback), state_pointer);
    }
    state.result
}

struct WindowSearch<'a> {
    match_mode: &'a str,
    target: &'a str,
    result: Option<HWND>,
}

unsafe extern "system" fn enum_windows_callback(handle: HWND, parameter: LPARAM) -> i32 {
    if handle.is_null() || unsafe { IsWindowVisible(handle) } == 0 {
        return 1;
    }
    let state = unsafe { &mut *(parameter as *mut WindowSearch<'_>) };
    if window_matches(handle, state.match_mode, state.target) {
        state.result = Some(handle);
        return 0;
    }
    1
}

fn window_matches(handle: HWND, match_mode: &str, target: &str) -> bool {
    let process_id = window_process_id(handle);
    match match_mode {
        "pid" => process_id.to_string() == target.trim(),
        "process_name" => process_executable_path(process_id)
            .as_deref()
            .and_then(process_name_from_path)
            .is_some_and(|process_name| contains_case_insensitive(process_name, target)),
        "executable_path" => process_executable_path(process_id)
            .as_deref()
            .is_some_and(|executable_path| contains_case_insensitive(executable_path, target)),
        "window_title" => contains_case_insensitive(&window_title(handle), target),
        _ => false,
    }
}

fn window_title(handle: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(handle);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0_u16; length as usize + 1];
        let copied = GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32);
        OsString::from_wide(&buffer[..copied.max(0) as usize])
            .to_string_lossy()
            .into_owned()
    }
}

fn window_process_id(handle: HWND) -> u32 {
    unsafe {
        let mut process_id = 0_u32;
        GetWindowThreadProcessId(handle, &mut process_id);
        process_id
    }
}

fn process_executable_path(process_id: u32) -> Option<String> {
    if process_id == 0 {
        return None;
    }

    unsafe {
        let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process_handle.is_null() {
            return None;
        }

        let mut buffer = vec![0_u16; 32_768];
        let mut buffer_len = buffer.len() as u32;
        let query_result =
            QueryFullProcessImageNameW(process_handle, 0, buffer.as_mut_ptr(), &mut buffer_len);
        CloseHandle(process_handle);

        if query_result == 0 || buffer_len == 0 {
            return None;
        }

        Some(
            OsString::from_wide(&buffer[..buffer_len as usize])
                .to_string_lossy()
                .into_owned(),
        )
    }
}

fn process_name_from_path(path: &str) -> Option<&str> {
    Path::new(path).file_name().and_then(|name| name.to_str())
}

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&needle.trim().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_process_name_from_windows_path() {
        assert_eq!(
            process_name_from_path(r"C:\Program Files\BaudBound\baudbound.exe"),
            Some("baudbound.exe")
        );
    }

    #[test]
    fn matches_case_insensitive_text() {
        assert!(contains_case_insensitive("BaudBound Runner", "runner"));
        assert!(contains_case_insensitive(
            "BaudBound Runner",
            "  BAUDBOUND  "
        ));
        assert!(!contains_case_insensitive("BaudBound Runner", "editor"));
    }
}
