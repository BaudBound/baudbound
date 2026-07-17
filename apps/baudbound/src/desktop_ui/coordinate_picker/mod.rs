mod session;
#[cfg(windows)]
mod windows;

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

#[cfg(windows)]
use crate::desktop_actions::screen_tools::{MonitorInfo, ScreenPixel};

pub(super) use session::CoordinatePickerState;

const PICKER_EVENT: &str = "coordinate-picker-finished";
const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Clone, Serialize)]
pub(super) struct CoordinatePickerStartPayload {
    monitor_count: usize,
    session_id: String,
}

#[cfg(windows)]
#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CoordinatePickerEvent {
    Selected { result: CoordinatePickerResult },
    Cancelled,
    Failed { message: String },
}

#[cfg(windows)]
#[derive(Clone, Serialize)]
struct CoordinatePickerResult {
    color: ScreenPixel,
    monitor: MonitorInfo,
    x: i32,
    y: i32,
}

#[tauri::command]
pub(super) async fn start_coordinate_picker<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, CoordinatePickerState>,
) -> Result<CoordinatePickerStartPayload, String> {
    #[cfg(windows)]
    {
        windows::start(&app, &state)
    }

    #[cfg(not(windows))]
    {
        let _ = (app, state);
        Err("The screen coordinate picker requires the Windows desktop runner.".to_owned())
    }
}

#[tauri::command]
pub(super) async fn select_coordinate_picker<R: Runtime>(
    app: AppHandle<R>,
    session_id: String,
    state: State<'_, CoordinatePickerState>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::select(&app, &state, &session_id)
    }

    #[cfg(not(windows))]
    {
        let _ = (app, session_id, state);
        Err("The screen coordinate picker requires the Windows desktop runner.".to_owned())
    }
}

#[tauri::command]
pub(super) async fn cancel_coordinate_picker<R: Runtime>(
    app: AppHandle<R>,
    session_id: String,
    state: State<'_, CoordinatePickerState>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::cancel(&app, &state, &session_id)
    }

    #[cfg(not(windows))]
    {
        let _ = (app, session_id, state);
        Err("The screen coordinate picker requires the Windows desktop runner.".to_owned())
    }
}
