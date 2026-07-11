use std::{ffi::OsString, os::windows::ffi::OsStringExt};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    },
};

pub(super) fn matching_window_title(process_id: u32, target: &str) -> Option<String> {
    let mut state = WindowSearch {
        process_id,
        result: None,
        target,
    };
    unsafe {
        let state_pointer = (&mut state as *mut WindowSearch).cast::<()>() as LPARAM;
        EnumWindows(Some(enum_windows_callback), state_pointer);
    }
    state.result
}

struct WindowSearch<'a> {
    process_id: u32,
    result: Option<String>,
    target: &'a str,
}

unsafe extern "system" fn enum_windows_callback(handle: HWND, parameter: LPARAM) -> i32 {
    if handle.is_null() || unsafe { IsWindowVisible(handle) } == 0 {
        return 1;
    }

    let state = unsafe { &mut *(parameter as *mut WindowSearch<'_>) };
    let mut process_id = 0_u32;
    unsafe { GetWindowThreadProcessId(handle, &mut process_id) };
    if process_id != state.process_id {
        return 1;
    }

    let title = window_title(handle);
    if contains_case_insensitive(&title, state.target)
        && state.result.as_ref().is_none_or(|current| title < *current)
    {
        state.result = Some(title);
    }
    1
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

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&needle.trim().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_matching_is_case_insensitive_and_trims_the_target() {
        assert!(contains_case_insensitive("BaudBound Runner", " runner "));
        assert!(!contains_case_insensitive("BaudBound Runner", "Editor"));
    }
}
