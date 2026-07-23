use baudbound_runtime::{RuntimeActionError, RuntimeActionRequest, RuntimeActionResult};
use serde_json::{Map, Number, Value};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM},
    System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
    UI::WindowsAndMessaging::{EnumWindows, IsWindowVisible},
};

use super::{
    contains_case_insensitive, process_executable_path, process_name_from_path, window_process_id,
    window_title,
};
use crate::desktop_actions::config::failed_error;

pub(in crate::desktop_actions) fn process_status_by_window_title(
    _request: &RuntimeActionRequest,
    target: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let output_data = find_window_by_title(target)
        .map_or_else(process_not_found_output, |handle| {
            process_status_output(handle, true)
        });
    Ok(RuntimeActionResult { output_data })
}

pub(in crate::desktop_actions) fn kill_process_by_window_title(
    request: &RuntimeActionRequest,
    target: &str,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let Some(handle) = find_window_by_title(target) else {
        return Err(failed_error(
            request,
            format!("no process window title contains {target:?}"),
        ));
    };
    let process_id = window_process_id(handle);
    let mut output_data = process_status_output(handle, true);

    unsafe {
        let process_handle = OpenProcess(PROCESS_TERMINATE, 0, process_id);
        if process_handle.is_null() {
            return Err(failed_error(
                request,
                format!("failed to open process {process_id} for termination"),
            ));
        }
        let terminated = TerminateProcess(process_handle, 1) != 0;
        CloseHandle(process_handle);
        if !terminated {
            return Err(failed_error(
                request,
                format!("failed to terminate process {process_id}"),
            ));
        }
    }

    output_data.insert("killed".to_owned(), Value::Bool(true));
    Ok(RuntimeActionResult { output_data })
}

fn process_status_output(handle: HWND, running: bool) -> Map<String, Value> {
    let process_id = window_process_id(handle);
    let executable_path = process_executable_path(process_id);
    let process_name = executable_path
        .as_deref()
        .and_then(process_name_from_path)
        .map(str::to_owned);
    Map::from_iter([
        ("running".to_owned(), Value::Bool(running)),
        (
            "state".to_owned(),
            Value::String(if running { "running" } else { "not_found" }.to_owned()),
        ),
        (
            "process_id".to_owned(),
            Value::Number(Number::from(process_id)),
        ),
        (
            "process_name".to_owned(),
            process_name.map_or(Value::Null, Value::String),
        ),
        (
            "executable_path".to_owned(),
            executable_path.map_or(Value::Null, Value::String),
        ),
    ])
}

fn process_not_found_output() -> Map<String, Value> {
    Map::from_iter([
        ("running".to_owned(), Value::Bool(false)),
        ("state".to_owned(), Value::String("not_found".to_owned())),
        ("process_id".to_owned(), Value::Null),
        ("process_name".to_owned(), Value::Null),
        ("executable_path".to_owned(), Value::Null),
    ])
}

fn find_window_by_title(target: &str) -> Option<HWND> {
    let mut state = WindowTitleSearch {
        target,
        result: None,
    };
    unsafe {
        let state_pointer = (&mut state as *mut WindowTitleSearch).cast::<()>() as LPARAM;
        EnumWindows(Some(enum_window_titles_callback), state_pointer);
    }
    state.result.map(|(_, handle)| handle)
}

struct WindowTitleSearch<'a> {
    target: &'a str,
    result: Option<(u32, HWND)>,
}

unsafe extern "system" fn enum_window_titles_callback(handle: HWND, parameter: LPARAM) -> i32 {
    if handle.is_null() || unsafe { IsWindowVisible(handle) } == 0 {
        return 1;
    }
    let state = unsafe { &mut *(parameter as *mut WindowTitleSearch<'_>) };
    if contains_case_insensitive(&window_title(handle), state.target) {
        let process_id = window_process_id(handle);
        if process_id != 0 && state.result.is_none_or(|(current, _)| process_id < current) {
            state.result = Some((process_id, handle));
        }
    }
    1
}
