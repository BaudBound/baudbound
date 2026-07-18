use anyhow::Error;
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

pub fn report_error(error: &Error) {
    tracing::error!(error = %error, "desktop startup failed");
    let message = format!("BaudBound could not start.\n\n{error:#}");
    let result = MessageDialog::new()
        .set_title("BaudBound could not start")
        .set_description(message)
        .set_level(MessageLevel::Error)
        .set_buttons(MessageButtons::Ok)
        .show();

    if result != MessageDialogResult::Ok {
        tracing::warn!("desktop startup error dialog closed without confirmation");
    }
}
