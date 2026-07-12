use std::env;

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

const INPUT_ACTION_TYPES: &[&str] = &[
    "action.keyboard",
    "action.keyboard.type_text",
    "action.mouse",
    "action.mouse.move",
];
const SCREEN_WINDOW_ACTION_TYPES: &[&str] = &[
    "action.pixel.get",
    "action.window.active",
    "action.window.focus",
];

#[derive(Clone, Copy, Serialize)]
pub struct DoctorCheck {
    pub action_types: &'static [&'static str],
    pub available: bool,
    pub label: &'static str,
    pub note: &'static str,
}

impl DoctorCheck {
    const fn new(
        available: bool,
        label: &'static str,
        action_types: &'static [&'static str],
        note: &'static str,
    ) -> Self {
        Self {
            action_types,
            available,
            label,
            note,
        }
    }
}

pub fn print_desktop_doctor(json: bool) -> Result<()> {
    let checks = desktop_doctor_checks();
    let supported = checks.iter().filter(|check| check.available).count();
    let unsupported = checks.len().saturating_sub(supported);
    let healthy = unsupported == 0;

    if json {
        let output = json!({
            "healthy": healthy,
            "os": env::consts::OS,
            "supported_count": supported,
            "unsupported_count": unsupported,
            "checks": checks
                .iter()
                .map(|check| json!({
                    "action_types": check.action_types,
                    "available": check.available,
                    "label": check.label,
                    "note": check.note,
                }))
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("BaudBound native desktop action doctor");
    println!("OS: {}", env::consts::OS);
    for check in checks {
        println!(
            "[{}] {}",
            if check.available {
                "supported"
            } else {
                "unsupported"
            },
            check.label
        );
        println!("  Nodes: {}", check.action_types.join(", "));
        println!("  {}", check.note);
    }
    Ok(())
}

pub fn desktop_doctor_checks() -> Vec<DoctorCheck> {
    vec![
        DoctorCheck::new(
            true,
            "Clipboard",
            &["action.clipboard"],
            "Uses the native clipboard backend through arboard. Requires a usable desktop/session clipboard provider at runtime.",
        ),
        DoctorCheck::new(
            true,
            "Desktop notifications",
            &["action.notification"],
            "Uses notify-rust. Platform notification services must be available at runtime.",
        ),
        DoctorCheck::new(
            true,
            "Message boxes",
            &["action.message_box"],
            "Uses native rfd dialogs. Requires a graphical desktop session.",
        ),
        DoctorCheck::new(
            true,
            "Audio playback",
            &["action.beep", "action.sound.play"],
            "Uses rodio and the system audio backend for generated tones and audio files. Requires an available output device.",
        ),
        DoctorCheck::new(
            cfg!(windows),
            "Keyboard and mouse automation",
            INPUT_ACTION_TYPES,
            if cfg!(windows) {
                "Uses native Windows input APIs through enigo. The OS may require accessibility/input permissions."
            } else {
                "Keyboard and mouse automation is restricted to Windows Desktop because the previous Linux backend supported X11 but not Wayland."
            },
        ),
        DoctorCheck::new(
            cfg!(windows),
            "Screen pixel and window APIs",
            SCREEN_WINDOW_ACTION_TYPES,
            if cfg!(windows) {
                "Get Pixel Color, Get Active Window, and Window Focus use native Win32 APIs."
            } else {
                "Get Pixel Color, Get Active Window, and Window Focus are unsupported on this platform until a native backend is implemented."
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_platform_accurate_input_and_window_groups() {
        let checks = desktop_doctor_checks();
        let input = checks
            .iter()
            .find(|check| check.label == "Keyboard and mouse automation")
            .expect("input diagnostic should exist");
        assert_eq!(input.available, cfg!(windows));
        assert_eq!(input.action_types, INPUT_ACTION_TYPES);

        let screen = checks
            .iter()
            .find(|check| check.label == "Screen pixel and window APIs")
            .expect("screen diagnostic should exist");
        assert_eq!(screen.action_types, SCREEN_WINDOW_ACTION_TYPES);
        assert!(!screen.action_types.contains(&"trigger.hotkey"));
    }
}
