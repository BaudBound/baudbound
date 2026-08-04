use tauri::{Runtime, WebviewWindow};

pub(super) fn enforce_private_input_policy<R, F>(
    window: &WebviewWindow<R>,
    completion: F,
) -> Result<(), String>
where
    R: Runtime,
    F: FnOnce(Result<(), String>) + Send + 'static,
{
    #[cfg(windows)]
    {
        window
            .with_webview(move |webview| completion(disable_webview2_form_storage(&webview)))
            .map_err(|error| format!("failed to schedule WebView2 input policy setup: {error}"))
    }

    #[cfg(not(windows))]
    {
        let _ = window;
        completion(Ok(()));
        Ok(())
    }
}

#[cfg(windows)]
fn disable_webview2_form_storage(webview: &tauri::webview::PlatformWebview) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
    use windows_core::{BOOL, Interface};

    let controller = webview.controller();
    let core = unsafe { controller.CoreWebView2() }
        .map_err(|error| format!("failed to access the WebView2 instance: {error}"))?;
    let settings = unsafe { core.Settings() }
        .map_err(|error| format!("failed to access WebView2 settings: {error}"))?;
    let settings: ICoreWebView2Settings4 = settings.cast().map_err(|error| {
        format!("the installed WebView2 runtime does not support autofill controls: {error}")
    })?;

    unsafe {
        settings
            .SetIsGeneralAutofillEnabled(false)
            .map_err(|error| format!("failed to disable WebView2 general autofill: {error}"))?;
        settings
            .SetIsPasswordAutosaveEnabled(false)
            .map_err(|error| format!("failed to disable WebView2 password saving: {error}"))?;
    }

    let mut general_autofill_enabled = BOOL::default();
    let mut password_autosave_enabled = BOOL::default();
    unsafe {
        settings
            .IsGeneralAutofillEnabled(&mut general_autofill_enabled)
            .map_err(|error| {
                format!("failed to verify WebView2 general autofill state: {error}")
            })?;
        settings
            .IsPasswordAutosaveEnabled(&mut password_autosave_enabled)
            .map_err(|error| format!("failed to verify WebView2 password saving state: {error}"))?;
    }
    if general_autofill_enabled.as_bool() || password_autosave_enabled.as_bool() {
        return Err(
            "WebView2 did not retain the disabled autofill and password-saving policy".to_owned(),
        );
    }
    Ok(())
}
