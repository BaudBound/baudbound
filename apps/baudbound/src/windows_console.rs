use crate::cli::LaunchMode;

pub fn detach_for_desktop_release(launch_mode: &LaunchMode) {
    let desktop_command = is_desktop_launch(launch_mode);

    #[cfg(all(windows, not(debug_assertions)))]
    if desktop_command {
        // The executable remains a console application so CLI commands keep normal
        // stdout and stderr. Desktop release launches do not need that console.
        unsafe {
            windows_sys::Win32::System::Console::FreeConsole();
        }
    }

    #[cfg(any(not(windows), debug_assertions))]
    let _ = desktop_command;
}

fn is_desktop_launch(launch_mode: &LaunchMode) -> bool {
    matches!(launch_mode, LaunchMode::Desktop { .. })
}

#[cfg(test)]
mod tests {
    use super::is_desktop_launch;
    use crate::cli::{Command, LaunchMode};

    #[test]
    fn detaches_only_for_the_desktop_ui() {
        assert!(is_desktop_launch(&LaunchMode::Desktop { autostart: false }));
        assert!(!is_desktop_launch(&LaunchMode::Command(Command::Status {
            json: false
        })));
    }
}
