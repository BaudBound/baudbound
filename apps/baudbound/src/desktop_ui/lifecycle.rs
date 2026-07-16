use std::time::Duration;

use anyhow::{Context, Result};
use tauri::{
    App, AppHandle, Manager, WebviewWindow, WindowEvent,
    image::Image,
    menu::{MenuBuilder, MenuEvent, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "baudbound";
const MENU_SHOW: &str = "show";
const MENU_START_RUNNER: &str = "start-runner";
const MENU_STOP_RUNNER: &str = "stop-runner";
const MENU_RELOAD_RUNNER: &str = "reload-runner";
const MENU_QUIT: &str = "quit";
const QUIT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

pub fn configure_desktop_lifecycle(app: &mut App, launched_from_autostart: bool) -> Result<()> {
    configure_close_to_tray(app);
    configure_tray(app)?;
    configure_initial_window_visibility(app, launched_from_autostart);
    Ok(())
}

fn configure_close_to_tray(app: &App) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let window_to_hide = window.clone();
    let app_handle = app.handle().clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let state = app_handle.state::<super::DesktopUiState>();
            let keep_running = state
                .desktop_settings
                .lock()
                .map(|settings| settings.keep_running_on_close)
                .unwrap_or(true);
            if keep_running {
                let _ = window_to_hide.hide();
            } else {
                log_runner_command(
                    "stop background runner before closing",
                    super::run_locked_message(&state, || {
                        state.background_runner.stop_and_wait(QUIT_SHUTDOWN_TIMEOUT)
                    }),
                );
                app_handle.exit(0);
            }
        }
    });
}

fn configure_initial_window_visibility(app: &App, launched_from_autostart: bool) {
    let state = app.state::<super::DesktopUiState>();
    let start_hidden = launched_from_autostart
        && state
            .desktop_settings
            .lock()
            .map(|settings| settings.start_minimized_to_tray)
            .unwrap_or(false);
    if start_hidden {
        return;
    }
    show_main_window(app.handle());
}

fn configure_tray(app: &App) -> Result<()> {
    let show_item = MenuItemBuilder::with_id(MENU_SHOW, "Show BaudBound").build(app)?;
    let start_runner_item =
        MenuItemBuilder::with_id(MENU_START_RUNNER, "Start background runner").build(app)?;
    let stop_runner_item =
        MenuItemBuilder::with_id(MENU_STOP_RUNNER, "Stop background runner").build(app)?;
    let reload_runner_item =
        MenuItemBuilder::with_id(MENU_RELOAD_RUNNER, "Reload background runner").build(app)?;
    let quit_item = MenuItemBuilder::with_id(MENU_QUIT, "Quit BaudBound").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .items(&[&start_runner_item, &stop_runner_item, &reload_runner_item])
        .separator()
        .item(&quit_item)
        .build()?;

    TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("BaudBound")
        .icon(load_tray_icon(app)?)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if is_left_click_release(&event) {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(handle_tray_menu_event)
        .build(app)
        .context("failed to create BaudBound tray icon")?;

    Ok(())
}

fn handle_tray_menu_event(app: &AppHandle, event: MenuEvent) {
    let state = app.state::<super::DesktopUiState>();
    match event.id().as_ref() {
        MENU_SHOW => show_main_window(app),
        MENU_START_RUNNER => log_runner_command(
            "start background runner",
            super::run_locked_message(&state, || super::start_background_runner_message(&state)),
        ),
        MENU_STOP_RUNNER => {
            log_runner_command(
                "stop background runner",
                super::run_locked_message(&state, || super::stop_background_runner_message(&state)),
            );
        }
        MENU_RELOAD_RUNNER => {
            log_runner_command(
                "stop background runner before reload",
                super::run_locked_message(&state, || {
                    state.background_runner.stop_and_wait(QUIT_SHUTDOWN_TIMEOUT)
                }),
            );
            log_runner_command(
                "restart background runner after reload",
                super::run_locked_message(&state, || {
                    super::start_background_runner_message(&state)
                }),
            );
        }
        MENU_QUIT => {
            log_runner_command(
                "stop background runner before quit",
                super::run_locked_message(&state, || {
                    state.background_runner.stop_and_wait(QUIT_SHUTDOWN_TIMEOUT)
                }),
            );
            app.exit(0);
        }
        _ => {}
    }
}

fn is_left_click_release(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    )
}

fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    restore_window(&window);
}

fn restore_window(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn log_runner_command<E>(action: &'static str, result: std::result::Result<String, E>)
where
    E: std::fmt::Display,
{
    match result {
        Ok(message) => tracing::info!(action, message),
        Err(error) => tracing::warn!(action, error = %error),
    }
}

fn load_tray_icon(app: &App) -> Result<Image<'static>> {
    if let Some(icon) = app.default_window_icon() {
        return Ok(icon.clone().to_owned());
    }

    Image::from_bytes(include_bytes!("../../icons/icon.ico"))
        .context("failed to load BaudBound tray icon")
}
