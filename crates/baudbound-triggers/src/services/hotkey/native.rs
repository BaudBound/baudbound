use std::{
    collections::BTreeMap,
    ptr,
    sync::mpsc::{self, SyncSender},
    thread::{self, JoinHandle},
    time::SystemTime,
};

use windows_sys::Win32::{
    Foundation::GetLastError,
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
            UnregisterHotKey, VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_INSERT,
            VK_LEFT, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
        },
        WindowsAndMessaging::{
            GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_HOTKEY, WM_QUIT,
        },
    },
};

use super::HotkeyService;
use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics,
    try_send_trigger_event,
};

pub struct NativeHotkeyService {
    binding_count: usize,
    thread_id: Option<u32>,
    worker: Option<JoinHandle<()>>,
}

impl NativeHotkeyService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            binding_count: 0,
            thread_id: None,
            worker: None,
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: SyncSender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let service = HotkeyService::from_registrations(registrations)?;
        if service.is_empty() {
            return Ok(Self::empty());
        }

        let bindings = service
            .registered_hotkeys()
            .into_iter()
            .enumerate()
            .map(|(index, expression)| {
                let id = i32::try_from(index + 1).map_err(|_| {
                    TriggerError::Failed(
                        "trigger.hotkey".to_owned(),
                        "too many native hotkey bindings".to_owned(),
                    )
                })?;
                parse_native_hotkey(id, expression)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let binding_count = bindings.len();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("baudbound-native-hotkeys".to_owned())
            .spawn(move || run_message_loop(service, bindings, sender, ready_sender))
            .map_err(|source| {
                TriggerError::Failed(
                    "trigger.hotkey".to_owned(),
                    format!("failed to start native hotkey thread: {source}"),
                )
            })?;
        let thread_id = match ready_receiver.recv() {
            Ok(Ok(thread_id)) => thread_id,
            Ok(Err(message)) => {
                let _ = worker.join();
                return Err(TriggerError::Failed("trigger.hotkey".to_owned(), message));
            }
            Err(_) => {
                let _ = worker.join();
                return Err(TriggerError::Failed(
                    "trigger.hotkey".to_owned(),
                    "native hotkey thread exited before initialization".to_owned(),
                ));
            }
        };

        Ok(Self {
            binding_count,
            thread_id: Some(thread_id),
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.binding_count == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.binding_count
    }

    #[must_use]
    pub fn diagnostics(&self) -> TriggerServiceDiagnostics {
        TriggerServiceDiagnostics::thread_backed(
            self.worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished()),
            self.binding_count,
            "native hotkey binding(s)",
        )
    }
}

impl Drop for NativeHotkeyService {
    fn drop(&mut self) {
        if let Some(thread_id) = self.thread_id.take() {
            // SAFETY: The worker creates its message queue before publishing this thread ID.
            unsafe {
                PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
            }
        }
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!("native hotkey worker panicked during shutdown");
        }
    }
}

#[derive(Debug)]
struct NativeBinding {
    expression: String,
    id: i32,
    modifiers: u32,
    virtual_key: u32,
}

fn run_message_loop(
    service: HotkeyService,
    bindings: Vec<NativeBinding>,
    sender: SyncSender<TriggerEvent>,
    ready: mpsc::SyncSender<Result<u32, String>>,
) {
    let thread_id = unsafe { GetCurrentThreadId() };
    let mut message = MSG::default();
    // SAFETY: A null window handle creates a thread message queue without removing a message.
    unsafe {
        PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }

    let mut expressions = BTreeMap::new();
    for binding in &bindings {
        // SAFETY: IDs are unique in this thread and virtual-key/modifier values are validated.
        let registered = unsafe {
            RegisterHotKey(
                ptr::null_mut(),
                binding.id,
                binding.modifiers,
                binding.virtual_key,
            )
        };
        if registered == 0 {
            let error = unsafe { GetLastError() };
            unregister_bindings(&bindings);
            let _ = ready.send(Err(format!(
                "failed to register hotkey {:?} (Win32 error {error}); it may already be in use",
                binding.expression
            )));
            return;
        }
        expressions.insert(binding.id, binding.expression.clone());
    }
    if ready.send(Ok(thread_id)).is_err() {
        unregister_bindings(&bindings);
        return;
    }

    loop {
        // SAFETY: message points to valid storage and this thread owns its message queue.
        let result = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        if message.message != WM_HOTKEY {
            continue;
        }
        let Ok(id) = i32::try_from(message.wParam) else {
            continue;
        };
        let Some(expression) = expressions.get(&id) else {
            continue;
        };
        match service.events_for_key(expression, SystemTime::now()) {
            Ok(events) => {
                for event in events {
                    try_send_trigger_event(&sender, event, "native hotkey");
                }
            }
            Err(error) => {
                tracing::error!(%error, hotkey = expression, "failed to build native hotkey event")
            }
        }
    }
    unregister_bindings(&bindings);
}

fn unregister_bindings(bindings: &[NativeBinding]) {
    for binding in bindings {
        // SAFETY: Unregistering an absent ID is harmless and the ID belongs to this thread.
        unsafe {
            UnregisterHotKey(ptr::null_mut(), binding.id);
        }
    }
}

fn parse_native_hotkey(id: i32, expression: &str) -> Result<NativeBinding, TriggerError> {
    let mut modifiers = MOD_NOREPEAT;
    let mut key = None;
    for part in expression.split('+') {
        match part {
            "Ctrl" => modifiers |= MOD_CONTROL,
            "Alt" => modifiers |= MOD_ALT,
            "Shift" => modifiers |= MOD_SHIFT,
            "Meta" => modifiers |= MOD_WIN,
            value if key.is_none() => key = Some(value),
            _ => {
                return Err(TriggerError::Failed(
                    "trigger.hotkey".to_owned(),
                    format!("hotkey {expression:?} contains multiple primary keys"),
                ));
            }
        }
    }
    let key = key.ok_or_else(|| {
        TriggerError::Failed(
            "trigger.hotkey".to_owned(),
            format!("hotkey {expression:?} has no primary key"),
        )
    })?;
    let virtual_key = virtual_key(key).ok_or_else(|| {
        TriggerError::Failed(
            "trigger.hotkey".to_owned(),
            format!("hotkey key {key:?} is not supported by the Windows native backend"),
        )
    })?;
    Ok(NativeBinding {
        expression: expression.to_owned(),
        id,
        modifiers,
        virtual_key,
    })
}

fn virtual_key(key: &str) -> Option<u32> {
    if key.len() == 1 {
        let byte = key.as_bytes()[0];
        if byte.is_ascii_uppercase() || byte.is_ascii_digit() {
            return Some(u32::from(byte));
        }
    }
    if let Some(number) = key
        .strip_prefix('F')
        .and_then(|value| value.parse::<u32>().ok())
        && (1..=24).contains(&number)
    {
        return Some(0x70 + number - 1);
    }
    Some(match key {
        "Escape" => u32::from(VK_ESCAPE),
        "Enter" => u32::from(VK_RETURN),
        "Space" => u32::from(VK_SPACE),
        "Tab" => u32::from(VK_TAB),
        "Backspace" => u32::from(VK_BACK),
        "Delete" => u32::from(VK_DELETE),
        "Insert" => u32::from(VK_INSERT),
        "Home" => u32::from(VK_HOME),
        "End" => u32::from(VK_END),
        "PageUp" => u32::from(VK_PRIOR),
        "PageDown" => u32::from(VK_NEXT),
        "ArrowUp" => u32::from(VK_UP),
        "ArrowDown" => u32::from(VK_DOWN),
        "ArrowLeft" => u32::from(VK_LEFT),
        "ArrowRight" => u32::from(VK_RIGHT),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_windows_hotkeys() {
        let hotkey = parse_native_hotkey(1, "Ctrl+Alt+B").expect("hotkey should parse");
        assert_eq!(hotkey.virtual_key, u32::from(b'B'));
        assert_ne!(hotkey.modifiers & MOD_CONTROL, 0);
        assert_ne!(hotkey.modifiers & MOD_ALT, 0);
        assert_eq!(virtual_key("F24"), Some(0x87));
        assert_eq!(virtual_key("ArrowLeft"), Some(u32::from(VK_LEFT)));
    }

    #[test]
    fn rejects_unsupported_windows_hotkey_keys() {
        let error = parse_native_hotkey(1, "Ctrl+MediaPlay")
            .expect_err("unsupported virtual key should fail");
        assert!(error.to_string().contains("not supported"));
    }
}
