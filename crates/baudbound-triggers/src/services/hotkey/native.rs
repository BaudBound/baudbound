mod state;

use std::{
    cell::RefCell,
    collections::BTreeMap,
    ptr,
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    thread::{self, JoinHandle},
    time::SystemTime,
};

use windows_sys::Win32::{
    Foundation::{GetLastError, LPARAM, LRESULT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
        LLKHF_LOWER_IL_INJECTED, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
        SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_APP, WM_KEYDOWN, WM_KEYUP,
        WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    },
};

use self::state::{KeyTransition, KeyboardState, NativeChord};
use super::{HotkeyService, modifier_virtual_keys, parse_hotkey};
use crate::{
    TriggerError, TriggerEvent, TriggerRegistration, TriggerServiceDiagnostics,
    try_send_trigger_event,
};

const WM_BAUDBOUND_RECONFIGURE_HOTKEYS: u32 = WM_APP + 1;

pub struct NativeHotkeyService {
    binding_count: usize,
    command_sender: Option<Sender<WorkerCommand>>,
    thread_id: Option<u32>,
    worker: Option<JoinHandle<()>>,
}

impl NativeHotkeyService {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            binding_count: 0,
            command_sender: None,
            thread_id: None,
            worker: None,
        }
    }

    pub fn start(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: SyncSender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        Self::start_or_reconfigure(registrations, sender, None)
    }

    pub fn start_or_reconfigure(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
        sender: SyncSender<TriggerEvent>,
        previous: Option<Self>,
    ) -> Result<Self, TriggerError> {
        let configuration = NativeConfiguration::from_registrations(registrations)?;
        if let Some(mut previous) = previous
            && previous
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished())
        {
            previous.reconfigure(configuration)?;
            return Ok(previous);
        }
        if configuration.binding_count == 0 {
            return Ok(Self::empty());
        }

        Self::start_worker(configuration, sender)
    }

    fn start_worker(
        configuration: NativeConfiguration,
        sender: SyncSender<TriggerEvent>,
    ) -> Result<Self, TriggerError> {
        let binding_count = configuration.binding_count;
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let (command_sender, command_receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("baudbound-native-hotkeys".to_owned())
            .spawn(move || {
                run_hook_loop(configuration, sender, command_receiver, ready_sender);
            })
            .map_err(|source| {
                hotkey_error(format!("failed to start native hotkey thread: {source}"))
            })?;
        let thread_id = match ready_receiver.recv() {
            Ok(Ok(thread_id)) => thread_id,
            Ok(Err(message)) => {
                let _ = worker.join();
                return Err(hotkey_error(message));
            }
            Err(_) => {
                let _ = worker.join();
                return Err(hotkey_error(
                    "native hotkey thread exited before initialization",
                ));
            }
        };

        Ok(Self {
            binding_count,
            command_sender: Some(command_sender),
            thread_id: Some(thread_id),
            worker: Some(worker),
        })
    }

    fn reconfigure(&mut self, configuration: NativeConfiguration) -> Result<(), TriggerError> {
        let binding_count = configuration.binding_count;
        let (acknowledge, acknowledged) = mpsc::sync_channel(1);
        self.command_sender
            .as_ref()
            .ok_or_else(|| hotkey_error("native hotkey command channel is unavailable"))?
            .send(WorkerCommand::Reconfigure {
                acknowledge,
                configuration,
            })
            .map_err(|_| hotkey_error("native hotkey worker stopped during reload"))?;
        let thread_id = self
            .thread_id
            .ok_or_else(|| hotkey_error("native hotkey thread ID is unavailable"))?;
        // SAFETY: The thread ID belongs to the live worker and its message queue is initialized.
        let posted =
            unsafe { PostThreadMessageW(thread_id, WM_BAUDBOUND_RECONFIGURE_HOTKEYS, 0, 0) };
        if posted == 0 {
            return Err(hotkey_error(format!(
                "failed to notify native hotkey worker during reload (Win32 error {})",
                unsafe { GetLastError() }
            )));
        }
        acknowledged
            .recv()
            .map_err(|_| hotkey_error("native hotkey worker stopped during reload"))?;
        self.binding_count = binding_count;
        Ok(())
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
            "native Windows hotkey binding(s)",
        )
    }
}

impl Drop for NativeHotkeyService {
    fn drop(&mut self) {
        self.command_sender.take();
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

struct NativeConfiguration {
    binding_count: usize,
    bindings: BTreeMap<NativeChord, String>,
    service: HotkeyService,
}

impl NativeConfiguration {
    fn from_registrations(
        registrations: impl IntoIterator<Item = TriggerRegistration>,
    ) -> Result<Self, TriggerError> {
        let service = HotkeyService::from_registrations(registrations)?;
        let mut bindings = BTreeMap::new();
        for expression in service.registered_hotkeys() {
            let parsed = parse_hotkey(expression).map_err(hotkey_error)?;
            let chord = NativeChord {
                modifiers: parsed.modifiers,
                virtual_keys: parsed.virtual_keys,
            };
            if bindings.insert(chord, parsed.expression).is_some() {
                return Err(hotkey_error(format!(
                    "hotkey {expression:?} duplicates another native key binding"
                )));
            }
        }
        Ok(Self {
            binding_count: bindings.len(),
            bindings,
            service,
        })
    }
}

enum WorkerCommand {
    Reconfigure {
        acknowledge: mpsc::SyncSender<()>,
        configuration: NativeConfiguration,
    },
}

struct HookContext {
    keyboard: KeyboardState,
    sender: SyncSender<TriggerEvent>,
    service: HotkeyService,
}

impl HookContext {
    fn new(
        configuration: NativeConfiguration,
        sender: SyncSender<TriggerEvent>,
    ) -> Result<Self, String> {
        Ok(Self {
            keyboard: KeyboardState::new(configuration.bindings, modifier_virtual_keys()?.clone()),
            sender,
            service: configuration.service,
        })
    }

    fn reconfigure(&mut self, configuration: NativeConfiguration) {
        self.keyboard.reconfigure(configuration.bindings);
        self.service = configuration.service;
    }

    fn process(&mut self, virtual_key: u32, transition: KeyTransition, injected: bool) {
        let Some(expression) = self.keyboard.process(virtual_key, transition, injected) else {
            return;
        };
        match self.service.events_for_key(&expression, SystemTime::now()) {
            Ok(events) => {
                for event in events {
                    try_send_trigger_event(&self.sender, event, "native hotkey");
                }
            }
            Err(error) => {
                tracing::error!(%error, hotkey = expression, "failed to build native hotkey event");
            }
        }
    }
}

thread_local! {
    static HOOK_CONTEXT: RefCell<Option<HookContext>> = const { RefCell::new(None) };
}

fn run_hook_loop(
    configuration: NativeConfiguration,
    sender: SyncSender<TriggerEvent>,
    commands: Receiver<WorkerCommand>,
    ready: mpsc::SyncSender<Result<u32, String>>,
) {
    let context = match HookContext::new(configuration, sender) {
        Ok(context) => context,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    HOOK_CONTEXT.with(|slot| slot.replace(Some(context)));

    let thread_id = unsafe { GetCurrentThreadId() };
    let mut message = MSG::default();
    // SAFETY: A null window handle creates this worker's message queue without removing a message.
    unsafe {
        PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
    // SAFETY: A null module name returns the module containing this executable's hook callback.
    let module = unsafe { GetModuleHandleW(ptr::null()) };
    if module.is_null() {
        HOOK_CONTEXT.with(|slot| slot.replace(None));
        let _ = ready.send(Err(format!(
            "failed to resolve runner module for native hotkeys (Win32 error {})",
            unsafe { GetLastError() }
        )));
        return;
    }
    // SAFETY: The callback has static lifetime and the thread pumps messages until unhooking it.
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0) };
    if hook.is_null() {
        HOOK_CONTEXT.with(|slot| slot.replace(None));
        let _ = ready.send(Err(format!(
            "failed to install native Windows keyboard hook (Win32 error {})",
            unsafe { GetLastError() }
        )));
        return;
    }
    if ready.send(Ok(thread_id)).is_err() {
        // SAFETY: This thread owns the successfully installed hook.
        unsafe {
            UnhookWindowsHookEx(hook);
        }
        HOOK_CONTEXT.with(|slot| slot.replace(None));
        return;
    }

    loop {
        // SAFETY: message points to valid storage and this thread owns its message queue.
        let result = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        if message.message == WM_BAUDBOUND_RECONFIGURE_HOTKEYS {
            apply_pending_commands(&commands);
        }
    }

    // SAFETY: This thread owns the hook and no callback can run after it is removed.
    unsafe {
        UnhookWindowsHookEx(hook);
    }
    HOOK_CONTEXT.with(|slot| slot.replace(None));
}

fn apply_pending_commands(commands: &Receiver<WorkerCommand>) {
    while let Ok(command) = commands.try_recv() {
        match command {
            WorkerCommand::Reconfigure {
                acknowledge,
                configuration,
            } => {
                HOOK_CONTEXT.with(|slot| {
                    let mut context = slot.borrow_mut();
                    context
                        .as_mut()
                        .expect("hotkey hook context must exist while its worker is running")
                        .reconfigure(configuration);
                });
                let _ = acknowledge.send(());
            }
        }
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION.cast_signed()
        && matches!(
            u32::try_from(wparam),
            Ok(WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP)
        )
    {
        // SAFETY: Windows supplies a valid KBDLLHOOKSTRUCT pointer for HC_ACTION keyboard events.
        let event = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let transition = if matches!(u32::try_from(wparam), Ok(WM_KEYUP | WM_SYSKEYUP)) {
            KeyTransition::Up
        } else {
            KeyTransition::Down
        };
        let injected = event.flags & (LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED) != 0;
        HOOK_CONTEXT.with(|slot| {
            if let Some(context) = slot.borrow_mut().as_mut() {
                context.process(event.vkCode, transition, injected);
            }
        });
    }

    // SAFETY: The hook never suppresses input and always forwards the event to the next hook.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

fn hotkey_error(message: impl Into<String>) -> TriggerError {
    TriggerError::Failed("trigger.hotkey".to_owned(), message.into())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn registration(key: &str) -> TriggerRegistration {
        TriggerRegistration {
            action_type: "trigger.hotkey".to_owned(),
            config: json!({ "key": key }),
            node_id: "n-hotkey".to_owned(),
            runner_type: "hotkey".to_owned(),
            script_id: "script".to_owned(),
            script_name: "Script".to_owned(),
        }
    }

    #[test]
    fn builds_unmodified_punctuation_numpad_and_media_bindings() {
        let configuration = NativeConfiguration::from_registrations([
            registration("A"),
            registration("Semicolon"),
            registration("Numpad7"),
            registration("MediaPlayPause"),
        ])
        .expect("supported native keys should parse");

        assert_eq!(configuration.binding_count, 4);
        assert!(configuration.bindings.contains_key(&NativeChord {
            modifiers: 0,
            virtual_keys: vec![65],
        }));
        assert!(configuration.bindings.contains_key(&NativeChord {
            modifiers: 0,
            virtual_keys: vec![179],
        }));
    }

    #[test]
    fn installs_reconfigures_and_removes_the_native_hook() {
        let (sender, _receiver) = mpsc::sync_channel(4);
        let service = NativeHotkeyService::start([registration("A")], sender.clone())
            .expect("native hook should install");
        let worker_thread = service.thread_id;

        let service = NativeHotkeyService::start_or_reconfigure(
            [registration("Ctrl+MediaPlayPause")],
            sender,
            Some(service),
        )
        .expect("native hook should reconfigure");

        assert_eq!(service.thread_id, worker_thread);
        assert_eq!(service.len(), 1);
        assert!(service.diagnostics().running);
        drop(service);
    }
}
