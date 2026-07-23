use tauri::{
    AppHandle, CursorIcon, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::desktop_actions::screen_tools::{self, MonitorInfo};

use super::{
    CoordinatePickerEvent, CoordinatePickerResult, CoordinatePickerStartPayload, MAIN_WINDOW_LABEL,
    PICKER_EVENT,
    session::{CoordinatePickerState, PickerSession},
};

pub(super) fn start<R: Runtime>(
    app: &AppHandle<R>,
    state: &CoordinatePickerState,
) -> Result<CoordinatePickerStartPayload, String> {
    let discovery = screen_tools::discover_monitors()?;
    if !discovery.supported || discovery.monitors.is_empty() {
        return Err("Windows did not report any connected monitors for the picker.".to_owned());
    }

    let session_id = state.reserve()?;
    let creation = create_windows(app, &session_id, &discovery.monitors);
    let (window_labels, focus_label) = match creation {
        Ok(created) => created,
        Err(error) => {
            state.clear(&session_id);
            return Err(error);
        }
    };

    if let Err(error) = state.set_windows(&session_id, window_labels.clone()) {
        destroy_windows_by_label(app, &window_labels);
        state.clear(&session_id);
        return Err(error);
    }

    if let Err(error) = show_windows(app, &window_labels, &focus_label) {
        if let Ok(session) = state.take(&session_id) {
            destroy_windows(app, &session);
        }
        restore_main_window(app);
        return Err(error);
    }

    Ok(CoordinatePickerStartPayload {
        monitor_count: discovery.monitors.len(),
        session_id,
    })
}

pub(super) fn select<R: Runtime>(
    app: &AppHandle<R>,
    state: &CoordinatePickerState,
    session_id: &str,
) -> Result<(), String> {
    let session = state.take(session_id)?;
    let selection = capture_selection(app, &session);
    destroy_windows(app, &session);
    restore_main_window(app);

    let event = match &selection {
        Ok(result) => CoordinatePickerEvent::Selected {
            result: result.clone(),
        },
        Err(message) => CoordinatePickerEvent::Failed {
            message: message.clone(),
        },
    };
    app.emit_to(MAIN_WINDOW_LABEL, PICKER_EVENT, event)
        .map_err(|error| format!("failed to return the coordinate picker result: {error}"))?;
    selection.map(|_| ())
}

pub(super) fn cancel<R: Runtime>(
    app: &AppHandle<R>,
    state: &CoordinatePickerState,
    session_id: &str,
) -> Result<(), String> {
    let session = state.take(session_id)?;
    destroy_windows(app, &session);
    restore_main_window(app);
    app.emit_to(
        MAIN_WINDOW_LABEL,
        PICKER_EVENT,
        CoordinatePickerEvent::Cancelled,
    )
    .map_err(|error| format!("failed to report picker cancellation: {error}"))
}

fn create_windows<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    monitors: &[MonitorInfo],
) -> Result<(Vec<String>, String), String> {
    let (cursor_x, cursor_y) = screen_tools::cursor_position()?;
    let focus_index = monitors
        .iter()
        .position(|monitor| monitor.bounds.contains(cursor_x, cursor_y))
        .unwrap_or(0);
    let mut labels = Vec::with_capacity(monitors.len());

    for (index, monitor) in monitors.iter().enumerate() {
        let label = format!("coordinate-picker-{session_id}-{index}");
        let url = WebviewUrl::App(format!("index.html?coordinatePicker={session_id}").into());
        let window = match WebviewWindowBuilder::new(app, &label, url)
            .title("Select a screen coordinate")
            .visible(false)
            .focused(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .closable(false)
            .build()
        {
            Ok(window) => window,
            Err(error) => {
                destroy_windows_by_label(app, &labels);
                return Err(format!(
                    "failed to create a coordinate picker overlay: {error}"
                ));
            }
        };

        let bounds = monitor.bounds;
        let geometry_result = window
            .set_position(PhysicalPosition::new(bounds.left, bounds.top))
            .and_then(|()| window.set_size(PhysicalSize::new(bounds.width, bounds.height)))
            .and_then(|()| window.set_cursor_icon(CursorIcon::Crosshair));
        if let Err(error) = geometry_result {
            let _ = window.destroy();
            destroy_windows_by_label(app, &labels);
            return Err(format!(
                "failed to place a coordinate picker overlay on {}: {error}",
                monitor.device_name
            ));
        }
        labels.push(label);
    }

    let focus_label = labels
        .get(focus_index)
        .cloned()
        .or_else(|| labels.first().cloned())
        .ok_or_else(|| "no coordinate picker overlay was created".to_owned())?;
    Ok((labels, focus_label))
}

fn show_windows<R: Runtime>(
    app: &AppHandle<R>,
    labels: &[String],
    focus_label: &str,
) -> Result<(), String> {
    for label in labels {
        let window = app
            .get_webview_window(label)
            .ok_or_else(|| format!("coordinate picker overlay {label:?} disappeared"))?;
        window
            .show()
            .map_err(|error| format!("failed to show coordinate picker overlay: {error}"))?;
    }

    app.get_webview_window(focus_label)
        .ok_or_else(|| "the focused coordinate picker overlay disappeared".to_owned())?
        .set_focus()
        .map_err(|error| format!("failed to focus the coordinate picker: {error}"))
}

fn capture_selection<R: Runtime>(
    app: &AppHandle<R>,
    session: &PickerSession,
) -> Result<CoordinatePickerResult, String> {
    let (x, y) = screen_tools::cursor_position()?;
    let monitor = screen_tools::validate_coordinate(x, y)?;
    hide_windows(app, session)?;
    screen_tools::flush_desktop_composition()?;
    let color = screen_tools::sample_pixel(x, y)?;
    Ok(CoordinatePickerResult {
        color,
        monitor,
        x,
        y,
    })
}

fn hide_windows<R: Runtime>(app: &AppHandle<R>, session: &PickerSession) -> Result<(), String> {
    for label in &session.window_labels {
        app.get_webview_window(label)
            .ok_or_else(|| format!("coordinate picker overlay {label:?} disappeared"))?
            .hide()
            .map_err(|error| format!("failed to hide a coordinate picker overlay: {error}"))?;
    }
    Ok(())
}

fn destroy_windows<R: Runtime>(app: &AppHandle<R>, session: &PickerSession) {
    destroy_windows_by_label(app, &session.window_labels);
}

fn destroy_windows_by_label<R: Runtime>(app: &AppHandle<R>, labels: &[String]) {
    for label in labels {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.destroy();
        }
    }
}

fn restore_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
