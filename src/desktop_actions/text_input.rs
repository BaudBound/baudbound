use std::{collections::BTreeMap, fmt, io, mem::size_of, ptr};

use thiserror::Error;
use windows_sys::Win32::{
    Foundation::SetLastError,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyState, GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD,
            KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC_EX, MapVirtualKeyExW,
            SendInput, VK_CAPITAL, VK_CONTROL, VK_MENU, VK_RETURN, VK_SHIFT, VK_TAB, VkKeyScanExW,
        },
        WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    },
};

const MODIFIER_SHIFT: u8 = 0b001;
const MODIFIER_CONTROL: u8 = 0b010;
const MODIFIER_ALT: u8 = 0b100;
const SUPPORTED_MODIFIERS: u8 = MODIFIER_SHIFT | MODIFIER_CONTROL | MODIFIER_ALT;

#[derive(Debug, Error)]
pub(super) enum TextInputError {
    #[error("text contains a NUL character at position {position}")]
    ContainsNul { position: usize },
    #[error("Windows did not provide a keyboard layout for the foreground application")]
    MissingForegroundLayout,
    #[error("the generated Windows input stream is too large")]
    InputStreamTooLarge,
    #[error("Windows accepted {sent} of {expected} keyboard events: {source}; {recovery}")]
    Injection {
        sent: u32,
        expected: u32,
        source: io::Error,
        recovery: RecoveryOutcome,
    },
}

#[derive(Debug)]
pub(super) enum RecoveryOutcome {
    NotRequired,
    Restored,
    Failed {
        sent: u32,
        expected: u32,
        source: io::Error,
    },
}

#[derive(Debug)]
enum RawInputError {
    InputStreamTooLarge,
    Injection {
        sent: u32,
        expected: u32,
        source: io::Error,
    },
}

impl fmt::Display for RecoveryOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRequired => formatter.write_str("keyboard state was not changed"),
            Self::Restored => formatter.write_str("keyboard state was restored"),
            Self::Failed {
                sent,
                expected,
                source,
            } => write!(
                formatter,
                "keyboard state restoration accepted {sent} of {expected} events: {source}"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyStroke {
    virtual_key: u16,
    scan_code: u16,
    modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Modifiers(u8);

impl Modifiers {
    const NONE: Self = Self(0);

    fn contains(self, modifier: u8) -> bool {
        self.0 & modifier != 0
    }

    fn virtual_keys(self) -> impl Iterator<Item = u16> {
        modifier_keys().into_iter().filter_map(
            move |(flag, key)| {
                if self.contains(flag) { Some(key) } else { None }
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyEvent {
    virtual_key: u16,
    scan_code: u16,
    flags: u32,
}

impl KeyEvent {
    fn key_down(virtual_key: u16, scan_code: u16) -> Self {
        Self {
            virtual_key,
            scan_code,
            flags: 0,
        }
    }

    fn key_up(virtual_key: u16, scan_code: u16) -> Self {
        Self {
            virtual_key,
            scan_code,
            flags: KEYEVENTF_KEYUP,
        }
    }

    fn unicode_down(code_unit: u16) -> Self {
        Self {
            virtual_key: 0,
            scan_code: code_unit,
            flags: KEYEVENTF_UNICODE,
        }
    }

    fn unicode_up(code_unit: u16) -> Self {
        Self {
            virtual_key: 0,
            scan_code: code_unit,
            flags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        }
    }

    fn into_input(self) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: self.virtual_key,
                    wScan: self.scan_code,
                    dwFlags: self.flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct InitialKeyboardState {
    modifiers: Modifiers,
    caps_lock: bool,
}

pub(super) fn type_text(text: &str) -> Result<(), TextInputError> {
    if text.is_empty() {
        return Ok(());
    }

    let layout = foreground_keyboard_layout()?;
    let initial_state = current_keyboard_state();
    let events = plan_text(text, initial_state, |character| {
        key_stroke_for_character(character, layout)
    })?;
    send_events(&events, initial_state)
}

fn foreground_keyboard_layout() -> Result<*mut core::ffi::c_void, TextInputError> {
    let foreground_window = unsafe { GetForegroundWindow() };
    if foreground_window.is_null() {
        return Err(TextInputError::MissingForegroundLayout);
    }
    let thread_id = unsafe { GetWindowThreadProcessId(foreground_window, ptr::null_mut()) };
    if thread_id == 0 {
        return Err(TextInputError::MissingForegroundLayout);
    }
    let layout = unsafe { GetKeyboardLayout(thread_id) };
    if layout.is_null() {
        return Err(TextInputError::MissingForegroundLayout);
    }
    Ok(layout)
}

fn current_keyboard_state() -> InitialKeyboardState {
    let mut modifiers = 0;
    for (flag, virtual_key) in modifier_keys() {
        if unsafe { GetAsyncKeyState(i32::from(virtual_key)) } < 0 {
            modifiers |= flag;
        }
    }
    InitialKeyboardState {
        modifiers: Modifiers(modifiers),
        caps_lock: unsafe { GetKeyState(i32::from(VK_CAPITAL)) } & 1 != 0,
    }
}

fn key_stroke_for_character(character: char, layout: *mut core::ffi::c_void) -> Option<KeyStroke> {
    let code_unit = u16::try_from(u32::from(character)).ok()?;
    let mapping = unsafe { VkKeyScanExW(code_unit, layout) };
    if mapping == -1 {
        return None;
    }
    let mapping = mapping as u16;
    let virtual_key = mapping & 0x00ff;
    let modifier_bits = (mapping >> 8) as u8;
    if virtual_key == 0 || modifier_bits & !SUPPORTED_MODIFIERS != 0 {
        return None;
    }
    Some(KeyStroke {
        virtual_key,
        scan_code: unsafe {
            MapVirtualKeyExW(u32::from(virtual_key), MAPVK_VK_TO_VSC_EX, layout) as u16
        },
        modifiers: Modifiers(modifier_bits),
    })
}

fn plan_text(
    text: &str,
    initial_state: InitialKeyboardState,
    mut map_character: impl FnMut(char) -> Option<KeyStroke>,
) -> Result<Vec<KeyEvent>, TextInputError> {
    let mut events = Vec::with_capacity(text.len().saturating_mul(4));
    let mut active_modifiers = initial_state.modifiers;
    transition_modifiers(&mut events, &mut active_modifiers, Modifiers::NONE);
    if initial_state.caps_lock {
        push_key_click(&mut events, VK_CAPITAL, 0);
    }

    let mut characters = text.chars().peekable();
    let mut position = 0;
    while let Some(character) = characters.next() {
        position += 1;
        if character == '\0' {
            return Err(TextInputError::ContainsNul { position });
        }

        if character == '\r' {
            if characters.next_if_eq(&'\n').is_some() {
                position += 1;
            }
            transition_modifiers(&mut events, &mut active_modifiers, Modifiers::NONE);
            push_key_click(&mut events, VK_RETURN, 0);
            continue;
        }
        if character == '\n' {
            transition_modifiers(&mut events, &mut active_modifiers, Modifiers::NONE);
            push_key_click(&mut events, VK_RETURN, 0);
            continue;
        }
        if character == '\t' {
            transition_modifiers(&mut events, &mut active_modifiers, Modifiers::NONE);
            push_key_click(&mut events, VK_TAB, 0);
            continue;
        }

        if let Some(key_stroke) = map_character(character) {
            transition_modifiers(&mut events, &mut active_modifiers, key_stroke.modifiers);
            push_key_click(&mut events, key_stroke.virtual_key, key_stroke.scan_code);
        } else {
            transition_modifiers(&mut events, &mut active_modifiers, Modifiers::NONE);
            let mut encoded = [0; 2];
            for &code_unit in character.encode_utf16(&mut encoded).iter() {
                events.push(KeyEvent::unicode_down(code_unit));
                events.push(KeyEvent::unicode_up(code_unit));
            }
        }
    }

    transition_modifiers(&mut events, &mut active_modifiers, Modifiers::NONE);
    if initial_state.caps_lock {
        push_key_click(&mut events, VK_CAPITAL, 0);
    }
    transition_modifiers(&mut events, &mut active_modifiers, initial_state.modifiers);
    Ok(events)
}

fn transition_modifiers(events: &mut Vec<KeyEvent>, active: &mut Modifiers, required: Modifiers) {
    for (flag, virtual_key) in modifier_keys().into_iter().rev() {
        if active.contains(flag) && !required.contains(flag) {
            events.push(KeyEvent::key_up(virtual_key, 0));
            active.0 &= !flag;
        }
    }
    for (flag, virtual_key) in modifier_keys() {
        if !active.contains(flag) && required.contains(flag) {
            events.push(KeyEvent::key_down(virtual_key, 0));
            active.0 |= flag;
        }
    }
}

const fn modifier_keys() -> [(u8, u16); 3] {
    [
        (MODIFIER_CONTROL, VK_CONTROL),
        (MODIFIER_ALT, VK_MENU),
        (MODIFIER_SHIFT, VK_SHIFT),
    ]
}

fn push_key_click(events: &mut Vec<KeyEvent>, virtual_key: u16, scan_code: u16) {
    events.push(KeyEvent::key_down(virtual_key, scan_code));
    events.push(KeyEvent::key_up(virtual_key, scan_code));
}

fn send_events(
    events: &[KeyEvent],
    initial_state: InitialKeyboardState,
) -> Result<(), TextInputError> {
    let (sent, expected, source) = match send_raw_events(events) {
        Ok(()) => return Ok(()),
        Err(RawInputError::InputStreamTooLarge) => {
            return Err(TextInputError::InputStreamTooLarge);
        }
        Err(RawInputError::Injection {
            sent,
            expected,
            source,
        }) => (sent, expected, source),
    };
    let recovery = recover_keyboard_state(events, sent, initial_state);
    Err(TextInputError::Injection {
        sent,
        expected,
        source,
        recovery,
    })
}

fn send_raw_events(events: &[KeyEvent]) -> Result<(), RawInputError> {
    let inputs = events
        .iter()
        .copied()
        .map(KeyEvent::into_input)
        .collect::<Vec<_>>();
    let expected = u32::try_from(inputs.len()).map_err(|_| RawInputError::InputStreamTooLarge)?;
    let input_size =
        i32::try_from(size_of::<INPUT>()).map_err(|_| RawInputError::InputStreamTooLarge)?;
    unsafe { SetLastError(0) };
    let sent = unsafe { SendInput(expected, inputs.as_ptr(), input_size) };
    if sent == expected {
        return Ok(());
    }
    Err(RawInputError::Injection {
        sent,
        expected,
        source: last_input_error(),
    })
}

fn last_input_error() -> io::Error {
    let source = io::Error::last_os_error();
    if source.raw_os_error() == Some(0) {
        io::Error::other(
            "Windows did not report an error code; input may be blocked by process integrity isolation",
        )
    } else {
        source
    }
}

fn recover_keyboard_state(
    events: &[KeyEvent],
    sent: u32,
    initial_state: InitialKeyboardState,
) -> RecoveryOutcome {
    if sent == 0 {
        return RecoveryOutcome::NotRequired;
    }

    // Reconstruct the state after the accepted prefix so no synthetic key remains held.
    let recovery = recovery_events(events, sent, initial_state);
    match send_raw_events(&recovery) {
        Ok(()) => RecoveryOutcome::Restored,
        Err(RawInputError::Injection {
            sent,
            expected,
            source,
        }) => RecoveryOutcome::Failed {
            sent,
            expected,
            source,
        },
        Err(RawInputError::InputStreamTooLarge) => RecoveryOutcome::Failed {
            sent: 0,
            expected: u32::MAX,
            source: io::Error::other("keyboard state restoration stream was too large"),
        },
    }
}

fn recovery_events(
    events: &[KeyEvent],
    sent: u32,
    initial_state: InitialKeyboardState,
) -> Vec<KeyEvent> {
    let mut down_keys = initial_state
        .modifiers
        .virtual_keys()
        .map(|virtual_key| (virtual_key, 0))
        .collect::<BTreeMap<_, _>>();
    let mut down_unicode = Vec::new();
    let mut caps_lock = initial_state.caps_lock;
    for event in events
        .iter()
        .take(usize::try_from(sent).unwrap_or(events.len()))
    {
        let is_key_up = event.flags & KEYEVENTF_KEYUP != 0;
        if event.flags & KEYEVENTF_UNICODE != 0 {
            if is_key_up {
                if let Some(index) = down_unicode
                    .iter()
                    .rposition(|code_unit| *code_unit == event.scan_code)
                {
                    down_unicode.remove(index);
                }
            } else {
                down_unicode.push(event.scan_code);
            }
            continue;
        }

        if is_key_up {
            down_keys.remove(&event.virtual_key);
        } else {
            down_keys.insert(event.virtual_key, event.scan_code);
            if event.virtual_key == VK_CAPITAL {
                caps_lock = !caps_lock;
            }
        }
    }

    let desired_modifiers = initial_state.modifiers.virtual_keys().collect::<Vec<_>>();
    let mut recovery = Vec::new();
    for code_unit in down_unicode.into_iter().rev() {
        recovery.push(KeyEvent::unicode_up(code_unit));
    }
    for (&virtual_key, &scan_code) in down_keys.iter().rev() {
        if !desired_modifiers.contains(&virtual_key) {
            recovery.push(KeyEvent::key_up(virtual_key, scan_code));
        }
    }
    if caps_lock != initial_state.caps_lock {
        push_key_click(&mut recovery, VK_CAPITAL, 0);
    }
    for virtual_key in desired_modifiers {
        if !down_keys.contains_key(&virtual_key) {
            recovery.push(KeyEvent::key_down(virtual_key, 0));
        }
    }

    recovery
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use windows_sys::Win32::{
        System::Threading::{AttachThreadInput, GetCurrentThreadId},
        UI::{
            Input::KeyboardAndMouse::{GetFocus, SetActiveWindow, SetFocus},
            WindowsAndMessaging::{
                BringWindowToTop, CW_USEDEFAULT, CreateWindowExW, DestroyWindow, DispatchMessageW,
                ES_AUTOHSCROLL, GetWindowTextLengthW, GetWindowTextW, MSG, PM_REMOVE, PeekMessageW,
                SW_SHOW, SetForegroundWindow, ShowWindow, TranslateMessage, WS_OVERLAPPEDWINDOW,
            },
        },
    };

    fn ascii_key_stroke(character: char) -> Option<KeyStroke> {
        if character == ' ' {
            return Some(KeyStroke {
                virtual_key: b' ' as u16,
                scan_code: 0x39,
                modifiers: Modifiers::NONE,
            });
        }
        if !character.is_ascii_alphabetic() {
            return None;
        }
        Some(KeyStroke {
            virtual_key: character.to_ascii_uppercase() as u16,
            scan_code: 1,
            modifiers: if character.is_ascii_uppercase() {
                Modifiers(MODIFIER_SHIFT)
            } else {
                Modifiers::NONE
            },
        })
    }

    fn neutral_state() -> InitialKeyboardState {
        InitialKeyboardState {
            modifiers: Modifiers::NONE,
            caps_lock: false,
        }
    }

    #[test]
    fn plans_every_character_of_the_reported_regression_in_order() {
        let events = plan_text("Hello from baudbound", neutral_state(), ascii_key_stroke)
            .expect("text should be planned");
        let typed_keys = events
            .iter()
            .filter(|event| {
                event.flags == 0
                    && !modifier_keys()
                        .iter()
                        .any(|(_, virtual_key)| *virtual_key == event.virtual_key)
            })
            .map(|event| event.virtual_key)
            .collect::<Vec<_>>();

        assert_eq!(
            typed_keys,
            "HELLO FROM BAUDBOUND"
                .bytes()
                .map(u16::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn emits_matching_utf16_press_and_release_events_for_supplementary_unicode() {
        let events = plan_text("\u{1f600}", neutral_state(), |_| None)
            .expect("unicode text should be planned");

        assert_eq!(
            events,
            vec![
                KeyEvent::unicode_down(0xd83d),
                KeyEvent::unicode_up(0xd83d),
                KeyEvent::unicode_down(0xde00),
                KeyEvent::unicode_up(0xde00),
            ]
        );
    }

    #[test]
    fn treats_windows_newline_as_one_enter_key() {
        let events =
            plan_text("a\r\nb", neutral_state(), ascii_key_stroke).expect("text should be planned");
        let return_presses = events
            .iter()
            .filter(|event| event.virtual_key == VK_RETURN && event.flags == 0)
            .count();

        assert_eq!(return_presses, 1);
    }

    #[test]
    fn restores_initial_modifier_and_caps_lock_state() {
        let initial_state = InitialKeyboardState {
            modifiers: Modifiers(MODIFIER_CONTROL | MODIFIER_SHIFT),
            caps_lock: true,
        };
        let events =
            plan_text("a", initial_state, ascii_key_stroke).expect("text should be planned");

        assert_eq!(events.first(), Some(&KeyEvent::key_up(VK_SHIFT, 0)));
        assert_eq!(events.get(1), Some(&KeyEvent::key_up(VK_CONTROL, 0)));
        assert_eq!(
            events
                .iter()
                .filter(|event| event.virtual_key == VK_CAPITAL && event.flags == 0)
                .count(),
            2
        );
        assert_eq!(
            &events[events.len() - 2..],
            &[
                KeyEvent::key_down(VK_CONTROL, 0),
                KeyEvent::key_down(VK_SHIFT, 0),
            ]
        );
    }

    #[test]
    fn rejects_nul_with_a_character_position() {
        let error = plan_text("ab\0c", neutral_state(), ascii_key_stroke)
            .expect_err("NUL must not be injected");

        assert!(matches!(error, TextInputError::ContainsNul { position: 3 }));
    }

    #[test]
    fn partial_injection_cleanup_releases_keys_and_restores_initial_state() {
        let initial_state = InitialKeyboardState {
            modifiers: Modifiers(MODIFIER_CONTROL),
            caps_lock: true,
        };
        let events =
            plan_text("a", initial_state, ascii_key_stroke).expect("text should be planned");
        let accepted_through_a_key_down = events
            .iter()
            .position(|event| event.virtual_key == b'A' as u16 && event.flags == 0)
            .expect("A key-down should be present")
            + 1;

        let recovery = recovery_events(
            &events,
            u32::try_from(accepted_through_a_key_down).expect("event index should fit"),
            initial_state,
        );

        assert_eq!(
            recovery,
            vec![
                KeyEvent::key_up(b'A' as u16, 1),
                KeyEvent::key_down(VK_CAPITAL, 0),
                KeyEvent::key_up(VK_CAPITAL, 0),
                KeyEvent::key_down(VK_CONTROL, 0),
            ]
        );
    }

    #[test]
    fn partial_unicode_injection_cleanup_releases_the_accepted_code_unit() {
        let events = plan_text("\u{1f600}", neutral_state(), |_| None)
            .expect("unicode text should be planned");

        assert_eq!(
            recovery_events(&events, 1, neutral_state()),
            vec![KeyEvent::unicode_up(0xd83d)]
        );
    }

    #[test]
    #[ignore = "requires an interactive Windows desktop and temporarily takes focus"]
    fn types_exact_text_into_a_native_windows_edit_control() {
        let class_name = "EDIT\0".encode_utf16().collect::<Vec<_>>();
        let window_name = [0_u16];
        let window = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_name.as_ptr(),
                WS_OVERLAPPEDWINDOW | ES_AUTOHSCROLL as u32,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                640,
                160,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        assert!(!window.is_null(), "native EDIT window should be created");

        let current_thread = unsafe { GetCurrentThreadId() };
        let foreground_thread =
            unsafe { GetWindowThreadProcessId(GetForegroundWindow(), ptr::null_mut()) };
        let attached = current_thread != foreground_thread;
        if attached {
            assert_ne!(
                unsafe { AttachThreadInput(current_thread, foreground_thread, 1) },
                0,
                "test should attach to the foreground input queue"
            );
        }
        unsafe {
            ShowWindow(window, SW_SHOW);
            BringWindowToTop(window);
            SetForegroundWindow(window);
            SetActiveWindow(window);
            SetFocus(window);
            assert_eq!(GetFocus(), window, "edit control should receive focus");
        }
        if attached {
            assert_ne!(
                unsafe { AttachThreadInput(current_thread, foreground_thread, 0) },
                0,
                "test should detach from the foreground input queue"
            );
        }

        let expected = "Hello from baudbound";
        type_text(expected).expect("native text injection should succeed");

        let deadline = Instant::now() + Duration::from_secs(2);
        let actual = loop {
            pump_window_messages();
            let current = window_text(window);
            if current == expected || Instant::now() >= deadline {
                break current;
            }
            std::thread::yield_now();
        };
        unsafe {
            DestroyWindow(window);
        }

        assert_eq!(actual, expected);
    }

    fn pump_window_messages() {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    fn window_text(window: *mut core::ffi::c_void) -> String {
        let length = unsafe { GetWindowTextLengthW(window) };
        assert!(length >= 0, "window text length should be available");
        let mut buffer = vec![0; usize::try_from(length).unwrap_or_default() + 1];
        let copied = unsafe {
            GetWindowTextW(
                window,
                buffer.as_mut_ptr(),
                i32::try_from(buffer.len()).expect("test text should fit in i32"),
            )
        };
        String::from_utf16(&buffer[..usize::try_from(copied).unwrap_or_default()])
            .expect("EDIT control should contain valid UTF-16")
    }
}
