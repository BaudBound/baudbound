use baudbound_actions::DesktopActionAdapter;
use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};

mod audio;
mod clipboard;
mod config;
mod dialogs;
#[cfg(windows)]
mod input;
#[cfg(windows)]
mod keyboard;
#[cfg(windows)]
mod mouse;
mod process;
mod screen;
#[cfg(not(windows))]
mod unsupported_input;
#[cfg(windows)]
mod windows_desktop;

use audio::{run_beep, run_sound_play};
use clipboard::run_clipboard;
use dialogs::{run_message_box, run_notification};
#[cfg(windows)]
use keyboard::{run_keyboard, run_keyboard_type_text};
#[cfg(windows)]
use mouse::{run_mouse_click, run_mouse_move};
use process::{run_kill_process_by_window_title, run_process_status_by_window_title};
use screen::{run_active_window, run_pixel_get, run_window_focus};
#[cfg(not(windows))]
use unsupported_input::{run_keyboard, run_keyboard_type_text, run_mouse_click, run_mouse_move};

#[derive(Debug, Default)]
pub struct SystemDesktopActionAdapter;

impl DesktopActionAdapter for SystemDesktopActionAdapter {
    fn beep(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_beep(request, context)
    }

    fn clipboard(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_clipboard(request)
    }

    fn message_box(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_message_box(request, context)
    }

    fn notification(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_notification(request)
    }

    fn sound_play(
        &self,
        request: &RuntimeActionRequest,
        context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_sound_play(request, context)
    }

    fn keyboard(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_keyboard(request)
    }

    fn keyboard_type_text(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_keyboard_type_text(request)
    }

    fn mouse_click(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_mouse_click(request)
    }

    fn mouse_move(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_mouse_move(request)
    }

    fn pixel_get(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_pixel_get(request)
    }

    fn active_window(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_active_window(request)
    }

    fn window_focus(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_window_focus(request)
    }

    fn process_status_by_window_title(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_process_status_by_window_title(request)
    }

    fn kill_process_by_window_title(
        &self,
        request: &RuntimeActionRequest,
        _context: &RuntimeContext,
    ) -> Result<RuntimeActionResult, RuntimeActionError> {
        run_kill_process_by_window_title(request)
    }
}

#[cfg(test)]
mod tests;
