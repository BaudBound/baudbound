use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::{Receiver, Sender, bounded, never, select};
use tauri::{
    AppHandle, Emitter, Manager, Runtime, State, UserAttentionType, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use baudbound_core::DesktopSettings;

use crate::desktop_actions::dialogs::{
    DesktopDialogContent, DesktopDialogError, DesktopDialogProvider, DesktopDialogRequest,
    DesktopDialogResponse, DesktopDialogSize,
};

const WINDOW_LABEL_PREFIX: &str = "desktop-dialog-";
const CONSOLE_WINDOW_LABEL: &str = "desktop-dialog-console";
const CONSOLE_CHANGED_EVENT: &str = "desktop-dialog-console-changed";

#[derive(Default)]
pub(super) struct DesktopDialogBroker {
    state: Mutex<BrokerState>,
}

#[derive(Default)]
struct BrokerState {
    active: Option<ActiveDialog>,
    app: Option<AppHandle>,
    options: DesktopDialogOptions,
    pending: VecDeque<PendingDialog>,
}

struct ActiveDialog {
    close_response: Option<DesktopDialogResponse>,
    id: String,
    label: String,
    request: DesktopDialogRequest,
    outcome_sender: Sender<BrokerOutcome>,
}

struct PendingDialog {
    close_response: Option<DesktopDialogResponse>,
    id: String,
    label: String,
    request: DesktopDialogRequest,
    outcome_sender: Sender<BrokerOutcome>,
}

enum BrokerOutcome {
    Cancelled,
    Failed(String),
    Response(DesktopDialogResponse),
}

#[derive(Clone, Copy, Default)]
pub(super) struct DesktopDialogOptions {
    console_always_on_top: bool,
    console_enabled: bool,
    console_focus_on_request: bool,
}

impl DesktopDialogOptions {
    pub(super) fn from_desktop_settings(settings: &DesktopSettings) -> Self {
        Self {
            console_always_on_top: settings.dialog_console_always_on_top,
            console_enabled: settings.dialog_console_enabled,
            console_focus_on_request: settings.dialog_console_focus_on_request,
        }
    }
}

impl DesktopDialogBroker {
    pub(super) fn connect(
        &self,
        app: AppHandle,
        options: DesktopDialogOptions,
    ) -> Result<(), String> {
        {
            let mut state = self.lock_state()?;
            state.app = Some(app.clone());
            state.options = options;
        }
        self.schedule_console_sync(app);
        Ok(())
    }

    pub(super) fn configure(&self, options: DesktopDialogOptions) -> Result<(), String> {
        let app = {
            let mut state = self.lock_state()?;
            state.options = options;
            state.app.clone()
        };
        if let Some(app) = app {
            self.schedule_console_sync(app);
        }
        Ok(())
    }

    pub(super) fn disconnect(&self) {
        let (active, pending) = self.state.lock().map_or((None, Vec::new()), |mut state| {
            state.app = None;
            let active = state.active.take();
            let pending = state.pending.drain(..).collect::<Vec<_>>();
            (active, pending)
        });
        if let Some(active) = active {
            let _ = active.outcome_sender.try_send(BrokerOutcome::Cancelled);
        }
        for pending in pending {
            let _ = pending.outcome_sender.try_send(BrokerOutcome::Cancelled);
        }
    }

    fn enqueue(&self, request: DesktopDialogRequest) -> Result<QueuedDialog, DesktopDialogError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| DesktopDialogError::Failed("broker state lock is poisoned".to_owned()))?;
        let app = state.app.clone().ok_or(DesktopDialogError::Unavailable)?;
        let id = random_request_id()?;
        let label = dialog_window_label(state.options, &id);
        let (outcome_sender, outcome_receiver) = bounded(1);
        let close_response = request.close_response();
        state.pending.push_back(PendingDialog {
            close_response,
            id: id.clone(),
            label: label.clone(),
            request,
            outcome_sender,
        });
        Ok(QueuedDialog {
            app,
            id,
            label,
            outcome_receiver,
        })
    }

    fn request_for_window(
        &self,
        request_id: &str,
        window_label: &str,
    ) -> Result<DesktopDialogRequest, String> {
        let state = self.lock_state()?;
        let active = authenticated_active(&state, request_id, window_label)?;
        Ok(active.request.clone())
    }

    fn console_state_for_window(
        &self,
        window_label: &str,
    ) -> Result<Option<DesktopDialogConsoleState>, String> {
        if window_label != CONSOLE_WINDOW_LABEL {
            return Err("this window is not the desktop dialog console".to_owned());
        }
        let state = self.lock_state()?;
        Ok(state
            .active
            .as_ref()
            .filter(|active| active.label == CONSOLE_WINDOW_LABEL)
            .map(|active| DesktopDialogConsoleState {
                pending_count: state.pending.len(),
                request: active.request.clone(),
                request_id: active.id.clone(),
            }))
    }

    fn submit_from_window(
        &self,
        request_id: &str,
        window_label: &str,
        response: DesktopDialogResponse,
    ) -> Result<(), String> {
        let state = self.lock_state()?;
        let active = authenticated_active(&state, request_id, window_label)?;
        active
            .outcome_sender
            .try_send(BrokerOutcome::Response(response))
            .map_err(|error| format!("desktop dialog response was already submitted: {error}"))
    }

    fn cancel_from_window(&self, request_id: &str, window_label: &str) -> Result<(), String> {
        let state = self.lock_state()?;
        let active = authenticated_active(&state, request_id, window_label)?;
        let response = active.request.close_response().ok_or_else(|| {
            "this message dialog requires an explicit Yes or No selection".to_owned()
        })?;
        active
            .outcome_sender
            .try_send(BrokerOutcome::Response(response))
            .map_err(|error| format!("desktop dialog response was already submitted: {error}"))
    }

    fn finish(&self, app: &AppHandle, request_id: &str, label: &str) {
        let (should_destroy, should_update_console) =
            self.state.lock().map_or((false, false), |mut state| {
                if let Some(index) = state
                    .pending
                    .iter()
                    .position(|pending| pending.id == request_id && pending.label == label)
                {
                    state.pending.remove(index);
                    return (
                        false,
                        state
                            .active
                            .as_ref()
                            .is_some_and(|active| active.label == CONSOLE_WINDOW_LABEL),
                    );
                }
                if state
                    .active
                    .as_ref()
                    .is_some_and(|active| active.id == request_id && active.label == label)
                {
                    let uses_console_window = label == CONSOLE_WINDOW_LABEL;
                    state.active = None;
                    return (!uses_console_window, false);
                }
                (false, false)
            });
        if should_destroy {
            destroy_window(app, label);
        }
        if should_update_console {
            let _ = emit_console_changed(app);
        }
        self.activate_next(app);
    }

    fn activate_next(&self, app: &AppHandle) {
        loop {
            let Some(pending) = self.promote_next() else {
                self.schedule_console_sync(app.clone());
                return;
            };
            let options = self
                .state
                .lock()
                .map(|state| state.options)
                .unwrap_or_default();
            let uses_console_window = pending.label == CONSOLE_WINDOW_LABEL;
            let result = if uses_console_window {
                show_console_dialog_window(app, &pending.id, options)
            } else {
                create_transient_window(app, &pending.id, &pending.label, pending.close_response)
            };
            if result.is_ok() {
                return;
            }
            let error = result.err().unwrap_or_else(|| {
                DesktopDialogError::Failed("dialog activation failed".to_owned())
            });
            let sender = self.state.lock().ok().and_then(|mut state| {
                if state
                    .active
                    .as_ref()
                    .is_some_and(|active| active.id == pending.id)
                {
                    state.active.take().map(|active| active.outcome_sender)
                } else {
                    None
                }
            });
            if let Some(sender) = sender {
                let _ = sender.try_send(BrokerOutcome::Failed(error.to_string()));
            }
        }
    }

    fn promote_next(&self) -> Option<ActiveDialog> {
        self.state.lock().ok().and_then(|mut state| {
            if state.active.is_some() {
                return None;
            }
            let pending = state.pending.pop_front()?;
            let active = ActiveDialog {
                close_response: pending.close_response,
                id: pending.id,
                label: pending.label,
                request: pending.request,
                outcome_sender: pending.outcome_sender,
            };
            state.active = Some(active.clone_for_return());
            Some(active)
        })
    }

    fn sync_console_window(&self, app: &AppHandle) -> Result<(), String> {
        let app_for_sync = app.clone();
        app.run_on_main_thread(move || {
            let result = (|| {
                let broker = app_for_sync.state::<Arc<DesktopDialogBroker>>();
                let (options, has_console_work) = broker
                    .console_sync_state()
                    .map_err(DesktopDialogError::Failed)?;
                if !options.console_enabled && !has_console_work {
                    if let Some(window) = app_for_sync.get_webview_window(CONSOLE_WINDOW_LABEL) {
                        window.destroy().map_err(|error| {
                            DesktopDialogError::Failed(format!(
                                "failed to destroy dialog console window: {error}"
                            ))
                        })
                    } else {
                        Ok(())
                    }
                } else {
                    ensure_console_window(&app_for_sync, None, options).and_then(|()| {
                        emit_console_changed(&app_for_sync).map_err(DesktopDialogError::Failed)
                    })
                }
            })();
            if let Err(error) = result {
                tracing::warn!(%error, "failed to synchronize desktop dialog console window");
            }
        })
        .map_err(|error| format!("failed to schedule dialog console synchronization: {error}"))
    }

    fn schedule_console_sync(&self, app: AppHandle) {
        if let Err(error) = self.sync_console_window(&app) {
            tracing::warn!(%error, "failed to schedule desktop dialog console synchronization");
        }
    }

    fn console_sync_state(&self) -> Result<(DesktopDialogOptions, bool), String> {
        let state = self.lock_state()?;
        let has_console_work = state
            .active
            .as_ref()
            .is_some_and(|active| active.label == CONSOLE_WINDOW_LABEL)
            || state
                .pending
                .iter()
                .any(|pending| pending.label == CONSOLE_WINDOW_LABEL);
        Ok((state.options, has_console_work))
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, BrokerState>, String> {
        self.state
            .lock()
            .map_err(|_| "desktop dialog broker state lock is poisoned".to_owned())
    }
}

impl ActiveDialog {
    fn clone_for_return(&self) -> Self {
        Self {
            close_response: self.close_response.clone(),
            id: self.id.clone(),
            label: self.label.clone(),
            request: self.request.clone(),
            outcome_sender: self.outcome_sender.clone(),
        }
    }
}

struct QueuedDialog {
    app: AppHandle,
    id: String,
    label: String,
    outcome_receiver: Receiver<BrokerOutcome>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesktopDialogConsoleState {
    pending_count: usize,
    request: DesktopDialogRequest,
    request_id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesktopDialogConsoleWindowState {
    fullscreen: bool,
}

impl DesktopDialogBroker {
    fn active_sender_for_window(
        &self,
        request_id: &str,
        label: &str,
    ) -> Option<Sender<BrokerOutcome>> {
        self.state.lock().ok().and_then(|state| {
            if state
                .active
                .as_ref()
                .is_some_and(|active| active.id == request_id && active.label == label)
            {
                state
                    .active
                    .as_ref()
                    .map(|active| active.outcome_sender.clone())
            } else {
                None
            }
        })
    }

    fn console_close_outcome(&self) -> Option<(DesktopDialogResponse, Sender<BrokerOutcome>)> {
        self.state.lock().ok().and_then(|state| {
            let active = state
                .active
                .as_ref()
                .filter(|active| active.label == CONSOLE_WINDOW_LABEL)?;
            active
                .request
                .close_response()
                .map(|response| (response, active.outcome_sender.clone()))
        })
    }
}

impl DesktopDialogProvider for DesktopDialogBroker {
    fn show_dialog(
        &self,
        mut request: DesktopDialogRequest,
        cancellation: &baudbound_runtime::RuntimeCancellationToken,
        timeout: Option<Duration>,
    ) -> Result<DesktopDialogResponse, DesktopDialogError> {
        if cancellation.is_cancelled() {
            return Err(DesktopDialogError::Cancelled);
        }
        let timeout_deadline = timeout.map(TimeoutDeadline::new).transpose()?;
        request.timeout_at_unix_ms = timeout_deadline.as_ref().map(|deadline| deadline.unix_ms);
        let queued = self.enqueue(request)?;
        self.activate_next(&queued.app);
        if cancellation.is_cancelled() {
            self.finish(&queued.app, &queued.id, &queued.label);
            return Err(DesktopDialogError::Cancelled);
        }

        let cancellation_subscription = cancellation.subscribe();
        let timeout_receiver = timeout_deadline.map_or_else(never, |deadline| {
            crossbeam_channel::after(deadline.instant.saturating_duration_since(Instant::now()))
        });
        let outcome = select! {
            recv(queued.outcome_receiver) -> outcome => match outcome {
                Ok(BrokerOutcome::Response(response)) => Ok(response),
                Ok(BrokerOutcome::Cancelled) => Err(DesktopDialogError::Cancelled),
                Ok(BrokerOutcome::Failed(message)) => Err(DesktopDialogError::Failed(message)),
                Err(_) => Err(DesktopDialogError::Failed("dialog closed without returning a response".to_owned())),
            },
            recv(cancellation_subscription.receiver()) -> _ => Err(DesktopDialogError::Cancelled),
            recv(timeout_receiver) -> _ => Ok(DesktopDialogResponse::button("timeout")),
        };
        self.finish(&queued.app, &queued.id, &queued.label);
        outcome
    }
}

struct TimeoutDeadline {
    instant: Instant,
    unix_ms: u64,
}

impl TimeoutDeadline {
    fn new(timeout: Duration) -> Result<Self, DesktopDialogError> {
        let instant = Instant::now().checked_add(timeout).ok_or_else(|| {
            DesktopDialogError::Failed(
                "dialog timeout exceeds the monotonic clock range".to_owned(),
            )
        })?;
        let deadline = SystemTime::now().checked_add(timeout).ok_or_else(|| {
            DesktopDialogError::Failed("dialog timeout exceeds the system clock range".to_owned())
        })?;
        let unix_ms = deadline
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                DesktopDialogError::Failed(format!(
                    "system clock is before the Unix epoch: {error}"
                ))
            })?
            .as_millis()
            .try_into()
            .map_err(|_| {
                DesktopDialogError::Failed(
                    "dialog timeout exceeds the supported timestamp range".to_owned(),
                )
            })?;
        Ok(Self { instant, unix_ms })
    }
}

#[tauri::command]
pub(super) fn fetch_desktop_dialog<R: Runtime>(
    broker: State<'_, Arc<DesktopDialogBroker>>,
    request_id: String,
    window: WebviewWindow<R>,
) -> Result<DesktopDialogRequest, String> {
    broker.request_for_window(&request_id, window.label())
}

#[tauri::command]
pub(super) fn fetch_desktop_dialog_console<R: Runtime>(
    broker: State<'_, Arc<DesktopDialogBroker>>,
    window: WebviewWindow<R>,
) -> Result<Option<DesktopDialogConsoleState>, String> {
    broker.console_state_for_window(window.label())
}

#[tauri::command]
pub(super) fn fetch_desktop_dialog_console_window_state<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<DesktopDialogConsoleWindowState, String> {
    authorize_console_window(window.label())?;
    window
        .is_fullscreen()
        .map(|fullscreen| DesktopDialogConsoleWindowState { fullscreen })
        .map_err(|error| format!("failed to read dialog console window state: {error}"))
}

#[tauri::command]
pub(super) fn set_desktop_dialog_console_fullscreen<R: Runtime>(
    fullscreen: bool,
    window: WebviewWindow<R>,
) -> Result<DesktopDialogConsoleWindowState, String> {
    authorize_console_window(window.label())?;
    window
        .set_fullscreen(fullscreen)
        .map_err(|error| format!("failed to change dialog console fullscreen state: {error}"))?;
    window
        .is_fullscreen()
        .map(|fullscreen| DesktopDialogConsoleWindowState { fullscreen })
        .map_err(|error| format!("failed to verify dialog console fullscreen state: {error}"))
}

#[tauri::command]
pub(super) fn submit_desktop_dialog<R: Runtime>(
    broker: State<'_, Arc<DesktopDialogBroker>>,
    request_id: String,
    response: DesktopDialogResponse,
    window: WebviewWindow<R>,
) -> Result<(), String> {
    broker.submit_from_window(&request_id, window.label(), response)
}

#[tauri::command]
pub(super) fn cancel_desktop_dialog<R: Runtime>(
    broker: State<'_, Arc<DesktopDialogBroker>>,
    request_id: String,
    window: WebviewWindow<R>,
) -> Result<(), String> {
    broker.cancel_from_window(&request_id, window.label())
}

#[tauri::command]
pub(super) async fn select_desktop_dialog_paths<R: Runtime>(
    broker: State<'_, Arc<DesktopDialogBroker>>,
    request_id: String,
    mode: String,
    multiple: bool,
    window: WebviewWindow<R>,
) -> Result<Vec<String>, String> {
    broker.request_for_window(&request_id, window.label())?;
    let paths = match (mode.as_str(), multiple) {
        ("file", true) => rfd::AsyncFileDialog::new()
            .pick_files()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|handle| handle.path().to_path_buf())
            .collect(),
        ("file", false) => rfd::AsyncFileDialog::new()
            .pick_file()
            .await
            .map(|handle| vec![handle.path().to_path_buf()])
            .unwrap_or_default(),
        ("folder", false) => rfd::AsyncFileDialog::new()
            .pick_folder()
            .await
            .map(|handle| vec![handle.path().to_path_buf()])
            .unwrap_or_default(),
        ("folder", true) => {
            return Err("folder selection does not support multiple paths".to_owned());
        }
        _ => return Err("unsupported desktop dialog path selection mode".to_owned()),
    };
    broker.request_for_window(&request_id, window.label())?;
    paths
        .into_iter()
        .map(|path| {
            path.into_os_string()
                .into_string()
                .map_err(|_| "selected path is not valid Unicode".to_owned())
        })
        .collect()
}

fn create_transient_window(
    app: &AppHandle,
    request_id: &str,
    label: &str,
    close_response: Option<DesktopDialogResponse>,
) -> Result<(), DesktopDialogError> {
    let (created_sender, created_receiver) = bounded(1);
    let app_for_creation = app.clone();
    let request_id = request_id.to_owned();
    let label = label.to_owned();
    app.run_on_main_thread(move || {
        let result = build_transient_window(&app_for_creation, &request_id, &label, close_response);
        let _ = created_sender.send(result);
    })
    .map_err(|error| {
        DesktopDialogError::Failed(format!("failed to schedule dialog creation: {error}"))
    })?;
    created_receiver.recv().map_err(|_| {
        DesktopDialogError::Failed("dialog creation task ended without a result".to_owned())
    })?
}

fn build_transient_window(
    app: &AppHandle,
    request_id: &str,
    label: &str,
    close_response: Option<DesktopDialogResponse>,
) -> Result<(), DesktopDialogError> {
    let request = {
        let broker = app.state::<Arc<DesktopDialogBroker>>();
        broker
            .request_for_window(request_id, label)
            .map_err(DesktopDialogError::Failed)?
    };
    let outcome_sender = {
        let broker = app.state::<Arc<DesktopDialogBroker>>();
        broker
            .active_sender_for_window(request_id, label)
            .ok_or_else(|| {
                DesktopDialogError::Failed("the desktop dialog is no longer active".to_owned())
            })?
    };
    let url = WebviewUrl::App(format!("index.html?desktopDialog={request_id}").into());
    let dialog_size = match &request.content {
        DesktopDialogContent::FormDialog { dialog_size, .. }
        | DesktopDialogContent::MessageDialog { dialog_size, .. } => *dialog_size,
    };
    let (width, height) = desktop_dialog_dimensions(dialog_size);
    let window = WebviewWindowBuilder::new(app, label, url)
        .title(format!("{} - BaudBound", request.title))
        .inner_size(width, height)
        .min_inner_size(420.0, 300.0)
        .max_inner_size(720.0, 760.0)
        .visible(false)
        .focused(false)
        .resizable(true)
        .maximizable(false)
        .minimizable(true)
        .skip_taskbar(false)
        .build()
        .map_err(|error| {
            DesktopDialogError::Failed(format!("failed to create dialog window: {error}"))
        })?;
    let event_outcome_sender = outcome_sender.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if let Some(response) = close_response.clone() {
                let _ = event_outcome_sender.try_send(BrokerOutcome::Response(response));
            }
        }
        WindowEvent::Destroyed => {
            let _ = event_outcome_sender.try_send(BrokerOutcome::Failed(
                "dialog renderer was destroyed before returning a response".to_owned(),
            ));
        }
        _ => {}
    });
    window.center().map_err(|error| {
        DesktopDialogError::Failed(format!("failed to center dialog window: {error}"))
    })?;
    let window_after_policy = window.clone();
    super::webview_policy::enforce_private_input_policy(&window, move |policy_result| {
        let result = policy_result.and_then(|()| {
            window_after_policy
                .show()
                .map_err(|error| format!("failed to show dialog window: {error}"))?;
            if window_after_policy.set_focus().is_err() {
                let _ = window_after_policy
                    .request_user_attention(Some(UserAttentionType::Informational));
            }
            Ok(())
        });
        if let Err(error) = result {
            let _ = outcome_sender.try_send(BrokerOutcome::Failed(error));
            let _ = window_after_policy.destroy();
        }
    })
    .map_err(DesktopDialogError::Failed)?;
    Ok(())
}

fn show_console_dialog_window(
    app: &AppHandle,
    request_id: &str,
    options: DesktopDialogOptions,
) -> Result<(), DesktopDialogError> {
    let (created_sender, created_receiver) = bounded(1);
    let app_for_creation = app.clone();
    let request_id = request_id.to_owned();
    app.run_on_main_thread(move || {
        let result = ensure_console_window(&app_for_creation, Some(&request_id), options);
        let _ = created_sender.send(result);
    })
    .map_err(|error| {
        DesktopDialogError::Failed(format!("failed to schedule dialog console update: {error}"))
    })?;
    let result = created_receiver.recv().map_err(|_| {
        DesktopDialogError::Failed("dialog console update task ended without a result".to_owned())
    })?;
    result?;
    emit_console_changed(app).map_err(DesktopDialogError::Failed)
}

fn ensure_console_window(
    app: &AppHandle,
    request_id: Option<&str>,
    options: DesktopDialogOptions,
) -> Result<(), DesktopDialogError> {
    if let Some(window) = app.get_webview_window(CONSOLE_WINDOW_LABEL) {
        configure_console_window_after_policy(app, &window, request_id, options)?;
        return Ok(());
    }
    let url = WebviewUrl::App("index.html?desktopDialogConsole=1".into());
    let window = WebviewWindowBuilder::new(app, CONSOLE_WINDOW_LABEL, url)
        .title(console_title(app, request_id))
        .inner_size(720.0, 760.0)
        .min_inner_size(420.0, 300.0)
        .visible(false)
        .focused(false)
        .resizable(true)
        .maximizable(true)
        .minimizable(true)
        .skip_taskbar(false)
        .always_on_top(options.console_always_on_top)
        .build()
        .map_err(|error| {
            DesktopDialogError::Failed(format!("failed to create dialog console window: {error}"))
        })?;
    let app_for_close = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let broker = app_for_close.state::<Arc<DesktopDialogBroker>>();
            if let Some((response, sender)) = broker.console_close_outcome() {
                let _ = sender.try_send(BrokerOutcome::Response(response));
            }
        }
        WindowEvent::Destroyed => {
            let broker = app_for_close.state::<Arc<DesktopDialogBroker>>();
            if let Some(sender) = broker.state.lock().ok().and_then(|state| {
                state.active.as_ref().and_then(|active| {
                    if active.label == CONSOLE_WINDOW_LABEL {
                        Some(active.outcome_sender.clone())
                    } else {
                        None
                    }
                })
            }) {
                let _ = sender.try_send(BrokerOutcome::Failed(
                    "dialog console window was destroyed before returning a response".to_owned(),
                ));
            }
        }
        _ => {}
    });
    window.center().map_err(|error| {
        DesktopDialogError::Failed(format!("failed to center dialog console window: {error}"))
    })?;
    configure_console_window_after_policy(app, &window, request_id, options)?;
    Ok(())
}

fn configure_console_window_after_policy<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    request_id: Option<&str>,
    options: DesktopDialogOptions,
) -> Result<(), DesktopDialogError> {
    let app_after_policy = app.clone();
    let window_after_policy = window.clone();
    let request_id = request_id.map(str::to_owned);
    super::webview_policy::enforce_private_input_policy(window, move |policy_result| {
        let result = policy_result
            .map_err(DesktopDialogError::Failed)
            .and_then(|()| {
                configure_console_window(&window_after_policy, request_id.as_deref(), options)
            });
        if let Err(error) = result {
            fail_console_window(&app_after_policy, &window_after_policy, error);
        }
    })
    .map_err(DesktopDialogError::Failed)?;
    Ok(())
}

fn fail_console_window<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    error: DesktopDialogError,
) {
    let message = error.to_string();
    tracing::error!(%message, "failed to initialize the dialog console window");
    let broker = app.state::<Arc<DesktopDialogBroker>>();
    if let Some(sender) = broker.state.lock().ok().and_then(|state| {
        state.active.as_ref().and_then(|active| {
            (active.label == CONSOLE_WINDOW_LABEL).then(|| active.outcome_sender.clone())
        })
    }) {
        let _ = sender.try_send(BrokerOutcome::Failed(message));
    }
    let _ = window.destroy();
}

fn configure_console_window<R: Runtime>(
    window: &WebviewWindow<R>,
    request_id: Option<&str>,
    options: DesktopDialogOptions,
) -> Result<(), DesktopDialogError> {
    window
        .set_title(&console_title_from_window(window, request_id))
        .map_err(|error| {
            DesktopDialogError::Failed(format!("failed to set dialog console title: {error}"))
        })?;
    window
        .set_always_on_top(options.console_always_on_top)
        .map_err(|error| {
            DesktopDialogError::Failed(format!("failed to set dialog console pinning: {error}"))
        })?;
    window.show().map_err(|error| {
        DesktopDialogError::Failed(format!("failed to show dialog console window: {error}"))
    })?;
    if request_id.is_some() && options.console_focus_on_request && window.set_focus().is_err() {
        let _ = window.request_user_attention(Some(UserAttentionType::Informational));
    }
    Ok(())
}

fn console_title<R: Runtime>(app: &tauri::AppHandle<R>, request_id: Option<&str>) -> String {
    request_id
        .and_then(|id| {
            let broker = app.state::<Arc<DesktopDialogBroker>>();
            broker.request_for_window(id, CONSOLE_WINDOW_LABEL).ok()
        })
        .map_or_else(
            || "BaudBound Dialog Console".to_owned(),
            |request| format!("{} - BaudBound", request.title),
        )
}

fn console_title_from_window<R: Runtime>(
    window: &WebviewWindow<R>,
    request_id: Option<&str>,
) -> String {
    let app = window.app_handle();
    console_title(app, request_id)
}

fn emit_console_changed(app: &AppHandle) -> Result<(), String> {
    app.emit_to(CONSOLE_WINDOW_LABEL, CONSOLE_CHANGED_EVENT, ())
        .map_err(|error| format!("failed to notify dialog console: {error}"))
}

fn destroy_window(app: &AppHandle, label: &str) {
    let app = app.clone();
    let label = label.to_owned();
    let _ = app.clone().run_on_main_thread(move || {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    });
}

fn desktop_dialog_dimensions(size: DesktopDialogSize) -> (f64, f64) {
    match size {
        DesktopDialogSize::Small => (480.0, 430.0),
        DesktopDialogSize::Medium => (600.0, 580.0),
        DesktopDialogSize::Large => (720.0, 760.0),
    }
}

fn dialog_window_label(options: DesktopDialogOptions, request_id: &str) -> String {
    if options.console_enabled {
        CONSOLE_WINDOW_LABEL.to_owned()
    } else {
        format!("{WINDOW_LABEL_PREFIX}{request_id}")
    }
}

fn authenticated_active<'a>(
    state: &'a BrokerState,
    request_id: &str,
    window_label: &str,
) -> Result<&'a ActiveDialog, String> {
    let active = state
        .active
        .as_ref()
        .ok_or_else(|| "the desktop dialog is no longer active".to_owned())?;
    if active.id != request_id || active.label != window_label {
        return Err("the desktop dialog request does not belong to this window".to_owned());
    }
    Ok(active)
}

fn authorize_console_window(window_label: &str) -> Result<(), String> {
    if window_label == CONSOLE_WINDOW_LABEL {
        Ok(())
    } else {
        Err("this window is not the desktop dialog console".to_owned())
    }
}

fn random_request_id() -> Result<String, DesktopDialogError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| {
        DesktopDialogError::Failed(format!("failed to generate dialog request ID: {error}"))
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_actions::dialogs::{MessageDialogButtons, MessageDialogVariant};

    fn request() -> DesktopDialogRequest {
        DesktopDialogRequest {
            requesting_script: "script".to_owned(),
            timeout_at_unix_ms: None,
            title: "Title".to_owned(),
            content: DesktopDialogContent::MessageDialog {
                buttons: MessageDialogButtons::OkCancel,
                dialog_size: DesktopDialogSize::Medium,
                message: "Message".to_owned(),
                variant: MessageDialogVariant::Info,
            },
        }
    }

    #[test]
    fn authenticates_request_and_window_together() {
        let (sender, _receiver) = bounded(1);
        let state = BrokerState {
            active: Some(ActiveDialog {
                id: "request".to_owned(),
                close_response: Some(DesktopDialogResponse::button("cancel")),
                label: "desktop-dialog-request".to_owned(),
                request: request(),
                outcome_sender: sender,
            }),
            ..BrokerState::default()
        };

        assert!(authenticated_active(&state, "request", "desktop-dialog-request").is_ok());
        assert!(authenticated_active(&state, "wrong", "desktop-dialog-request").is_err());
        assert!(authenticated_active(&state, "request", "desktop-dialog-wrong").is_err());
    }

    #[test]
    fn request_ids_are_random_and_fixed_width() {
        let first = random_request_id().expect("request ID should be generated");
        let second = random_request_id().expect("request ID should be generated");
        assert_eq!(first.len(), 32);
        assert_ne!(first, second);
    }

    #[test]
    fn authorizes_only_the_stable_dialog_console_window() {
        assert!(authorize_console_window(CONSOLE_WINDOW_LABEL).is_ok());
        assert!(authorize_console_window("desktop-dialog-request").is_err());
        assert!(authorize_console_window("main").is_err());
    }

    #[test]
    fn console_never_projects_a_transient_window_request() {
        let (sender, _receiver) = bounded(1);
        let broker = DesktopDialogBroker {
            state: Mutex::new(BrokerState {
                active: Some(ActiveDialog {
                    close_response: Some(DesktopDialogResponse::button("cancel")),
                    id: "request".to_owned(),
                    label: "desktop-dialog-request".to_owned(),
                    request: request(),
                    outcome_sender: sender,
                }),
                ..BrokerState::default()
            }),
        };

        assert!(
            broker
                .console_state_for_window(CONSOLE_WINDOW_LABEL)
                .expect("console state should be readable")
                .is_none()
        );
    }

    #[test]
    fn maps_every_dialog_size_to_its_window_dimensions() {
        assert_eq!(
            desktop_dialog_dimensions(DesktopDialogSize::Small),
            (480.0, 430.0)
        );
        assert_eq!(
            desktop_dialog_dimensions(DesktopDialogSize::Medium),
            (600.0, 580.0)
        );
        assert_eq!(
            desktop_dialog_dimensions(DesktopDialogSize::Large),
            (720.0, 760.0)
        );
    }

    #[test]
    fn pending_dialogs_do_not_replace_the_active_dialog() {
        let (active_sender, _active_receiver) = bounded(1);
        let (pending_sender, _pending_receiver) = bounded(1);
        let state = BrokerState {
            active: Some(ActiveDialog {
                close_response: Some(DesktopDialogResponse::button("cancel")),
                id: "active".to_owned(),
                label: "desktop-dialog-active".to_owned(),
                request: request(),
                outcome_sender: active_sender,
            }),
            pending: VecDeque::from([PendingDialog {
                close_response: Some(DesktopDialogResponse::button("cancel")),
                id: "pending".to_owned(),
                label: "desktop-dialog-pending".to_owned(),
                request: request(),
                outcome_sender: pending_sender,
            }]),
            ..BrokerState::default()
        };

        assert_eq!(
            state.active.as_ref().map(|active| active.id.as_str()),
            Some("active")
        );
        assert_eq!(state.pending.len(), 1);
    }

    #[test]
    fn console_mode_uses_one_stable_window_label_for_every_queued_dialog() {
        let console_options = DesktopDialogOptions {
            console_enabled: true,
            ..DesktopDialogOptions::default()
        };

        assert_eq!(
            dialog_window_label(console_options, "first"),
            CONSOLE_WINDOW_LABEL
        );
        assert_eq!(
            dialog_window_label(console_options, "second"),
            CONSOLE_WINDOW_LABEL
        );
        assert_eq!(
            dialog_window_label(DesktopDialogOptions::default(), "request"),
            "desktop-dialog-request"
        );
    }

    #[test]
    fn accepts_exactly_one_authenticated_response() {
        let (sender, receiver) = bounded(1);
        let broker = DesktopDialogBroker {
            state: Mutex::new(BrokerState {
                active: Some(ActiveDialog {
                    id: "request".to_owned(),
                    close_response: Some(DesktopDialogResponse::button("cancel")),
                    label: "desktop-dialog-request".to_owned(),
                    request: request(),
                    outcome_sender: sender,
                }),
                ..BrokerState::default()
            }),
        };

        broker
            .submit_from_window(
                "request",
                "desktop-dialog-request",
                DesktopDialogResponse::button("ok"),
            )
            .expect("the first response should be accepted");
        assert!(
            broker
                .submit_from_window(
                    "request",
                    "desktop-dialog-request",
                    DesktopDialogResponse::button("cancel"),
                )
                .is_err(),
            "a duplicate response must be rejected"
        );
        assert!(matches!(
            receiver.try_recv(),
            Ok(BrokerOutcome::Response(DesktopDialogResponse { button, .. })) if button == "ok"
        ));
    }

    #[test]
    fn snapshots_console_close_behavior_from_one_active_request() {
        let (sender, _receiver) = bounded(1);
        let broker = DesktopDialogBroker {
            state: Mutex::new(BrokerState {
                active: Some(ActiveDialog {
                    close_response: Some(DesktopDialogResponse::button("cancel")),
                    id: "request".to_owned(),
                    label: CONSOLE_WINDOW_LABEL.to_owned(),
                    request: request(),
                    outcome_sender: sender,
                }),
                ..BrokerState::default()
            }),
        };

        let (response, _sender) = broker
            .console_close_outcome()
            .expect("an Ok/Cancel dialog should have a close outcome");
        assert_eq!(response.button, "cancel");

        let mut state = broker
            .state
            .lock()
            .expect("broker state should be available");
        let active = state
            .active
            .as_mut()
            .expect("active request should remain present");
        active.request.content = DesktopDialogContent::MessageDialog {
            buttons: MessageDialogButtons::YesNo,
            dialog_size: DesktopDialogSize::Medium,
            message: "Message".to_owned(),
            variant: MessageDialogVariant::Info,
        };
        drop(state);
        assert!(broker.console_close_outcome().is_none());
    }
}
