use super::options::ServeOptions;

pub(super) fn should_exit_idle_service(options: &ServeOptions) -> bool {
    !options.schedules_enabled
        && !options.file_watch_enabled
        && !options.hotkey_stdin_enabled
        && !options.process_watch_enabled
        && !options.serial_enabled
        && !options.startup_enabled
        && !options.webhooks_enabled
        && !options.websockets_enabled
}

pub(super) fn print_idle_service_explanation(options: &ServeOptions) {
    if options.startup_enabled {
        println!("No enabled startup triggers found.");
    } else {
        println!("Startup triggers are disabled in runner config.");
    }
    if options.schedules_enabled {
        println!("No enabled schedule triggers found.");
    } else {
        println!("Schedule triggers are disabled in runner config.");
    }
    if options.file_watch_enabled {
        println!("No enabled file watch triggers found.");
    } else {
        println!("File watch triggers are disabled in runner config.");
    }
    if options.process_watch_enabled {
        println!("No enabled process started triggers found.");
    } else {
        println!("Process started triggers are disabled in runner config.");
    }
    if options.hotkey_stdin_enabled {
        println!("No enabled hotkey triggers found.");
    }
    if !options.webhooks_enabled {
        println!("Webhook listener is disabled. Enable it in config or pass --webhooks.");
    }
    if !options.websockets_enabled {
        println!("WebSocket listener is disabled. Enable it in config or pass --websockets.");
    }
}
