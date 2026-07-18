use crate::cli::Command;

pub fn detach_for_desktop_release(command: &Command) {
    let desktop_command = is_desktop_command(command);

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

fn is_desktop_command(command: &Command) -> bool {
    matches!(command, Command::Ui { .. })
}

#[cfg(test)]
mod tests {
    use super::is_desktop_command;
    use crate::cli::Command;

    #[test]
    fn detaches_only_for_the_desktop_ui() {
        assert!(is_desktop_command(&Command::Ui { autostart: false }));
        assert!(!is_desktop_command(&Command::Status { json: false }));
    }
}
