use super::options::ServeOptions;
use crate::console;

pub(super) fn should_exit_idle_service(options: &ServeOptions) -> bool {
    !options.schedules_enabled
        && !options.file_watch_enabled
        && !options.hotkeys_enabled
        && !options.hotkey_stdin_enabled
        && !options.process_watch_enabled
        && !options.serial_enabled
        && !options.startup_enabled
        && !options.webhooks_enabled
        && !options.websockets_enabled
}

pub(super) fn print_idle_service_explanation(options: &ServeOptions) {
    if options.startup_enabled {
        console::info(format_args!("No enabled startup triggers found."));
    } else {
        console::info(format_args!(
            "Startup triggers are disabled in runner config."
        ));
    }
    if options.schedules_enabled {
        console::info(format_args!("No enabled schedule triggers found."));
    } else {
        console::info(format_args!(
            "Schedule triggers are disabled in runner config."
        ));
    }
    if options.file_watch_enabled {
        console::info(format_args!("No enabled file watch triggers found."));
    } else {
        console::info(format_args!(
            "File watch triggers are disabled in runner config."
        ));
    }
    if options.process_watch_enabled {
        console::info(format_args!("No enabled process started triggers found."));
    } else {
        console::info(format_args!(
            "Process started triggers are disabled in runner config."
        ));
    }
    if options.hotkey_stdin_enabled {
        console::info(format_args!("No enabled stdin hotkey triggers found."));
    }
    if cfg!(windows) && options.hotkeys_enabled {
        console::info(format_args!("No enabled native hotkey triggers found."));
    } else if cfg!(windows) {
        console::info(format_args!(
            "Native hotkey triggers are disabled in runner config."
        ));
    }
    if !options.webhooks_enabled {
        console::info(format_args!(
            "Webhook listener is disabled. Enable it in config or pass --webhooks."
        ));
    }
    if !options.websockets_enabled {
        console::info(format_args!(
            "WebSocket listener is disabled. Enable it in config or pass --websockets."
        ));
    }
}
