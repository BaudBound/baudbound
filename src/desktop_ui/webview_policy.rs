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

/// Makes the webview behave like an application window rather than a browser.
///
/// The webview ships with browser chrome that has no meaning here: a find bar,
/// reload, print, page zoom and a right-click menu offering to inspect the
/// page. WebView2 exposes these as settings, which is more reliable than
/// intercepting key presses and leaves the standard editing shortcuts alone.
pub(super) fn enforce_desktop_chrome_policy<R, F>(
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
            .with_webview(move |webview| completion(disable_webview2_browser_chrome(&webview)))
            .map_err(|error| format!("failed to schedule WebView2 chrome policy setup: {error}"))
    }

    #[cfg(not(windows))]
    {
        // Other platforms are covered by the interface's own key and menu
        // handling, which runs everywhere.
        let _ = window;
        completion(Ok(()));
        Ok(())
    }
}

#[cfg(windows)]
fn disable_webview2_browser_chrome(
    webview: &tauri::webview::PlatformWebview,
) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings;
    use windows_core::BOOL;

    let controller = webview.controller();
    let core = unsafe { controller.CoreWebView2() }
        .map_err(|error| format!("failed to access the WebView2 instance: {error}"))?;
    let settings: ICoreWebView2Settings = unsafe { core.Settings() }
        .map_err(|error| format!("failed to access WebView2 settings: {error}"))?;

    unsafe {
        settings
            .SetAreDefaultContextMenusEnabled(false)
            .map_err(|error| format!("failed to disable the WebView2 context menu: {error}"))?;
        settings
            .SetIsZoomControlEnabled(false)
            .map_err(|error| format!("failed to disable WebView2 zoom control: {error}"))?;
    }

    // Browser accelerators cover the find bar, reload, print and view source.
    // They also cover the developer tools, so a debug build keeps them.
    #[cfg(not(debug_assertions))]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows_core::Interface;

        let accelerators: ICoreWebView2Settings3 = settings.cast().map_err(|error| {
            format!("the installed WebView2 runtime does not support accelerator policy: {error}")
        })?;
        unsafe {
            accelerators
                .SetAreBrowserAcceleratorKeysEnabled(false)
                .map_err(|error| {
                    format!("failed to disable WebView2 browser accelerator keys: {error}")
                })?;
        }
        let mut accelerators_enabled = BOOL::default();
        unsafe {
            accelerators
                .AreBrowserAcceleratorKeysEnabled(&mut accelerators_enabled)
                .map_err(|error| {
                    format!("failed to verify WebView2 accelerator key state: {error}")
                })?;
        }
        if accelerators_enabled.as_bool() {
            return Err("WebView2 did not retain the disabled browser accelerator keys".to_owned());
        }
    }

    let mut context_menus_enabled = BOOL::default();
    let mut zoom_control_enabled = BOOL::default();
    unsafe {
        settings
            .AreDefaultContextMenusEnabled(&mut context_menus_enabled)
            .map_err(|error| {
                format!("failed to verify the WebView2 context menu state: {error}")
            })?;
        settings
            .IsZoomControlEnabled(&mut zoom_control_enabled)
            .map_err(|error| {
                format!("failed to verify the WebView2 zoom control state: {error}")
            })?;
    }
    if context_menus_enabled.as_bool() || zoom_control_enabled.as_bool() {
        return Err("WebView2 did not retain the disabled context menu and zoom policy".to_owned());
    }
    Ok(())
}
