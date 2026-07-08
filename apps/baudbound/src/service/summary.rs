use baudbound_storage::FilesystemScriptStore;

use super::triggers::TriggerServices;

pub(super) fn print_service_summary(services: &TriggerServices, store: &FilesystemScriptStore) {
    print_count(
        !services.schedules.is_empty(),
        services.schedules.len(),
        "schedule trigger",
        Some(store),
    );
    print_count(
        !services.file_watch_service.is_empty(),
        services.file_watch_service.len(),
        "file watch trigger",
        Some(store),
    );
    print_count(
        !services.process_started_service.is_empty(),
        services.process_started_service.len(),
        "process started listener",
        None,
    );
    print_count(
        !services.startup.is_empty(),
        services.startup.len(),
        "startup trigger",
        None,
    );
    if let Some(host) = &services.webhook_host {
        print_count(true, host.service.len(), "webhook trigger", None);
    }
    print_count(
        !services.websocket_service.is_empty(),
        services.websocket_service.len(),
        "WebSocket trigger",
        None,
    );
    print_count(
        !services.serial_input_service.is_empty(),
        services.serial_input_service.len(),
        "serial input trigger",
        None,
    );
    if !services.hotkey_service.is_empty() {
        println!(
            "Serving {} desktop hotkey trigger{} from stdin.",
            services.hotkey_service.len(),
            plural(services.hotkey_service.len()),
        );
    }
}

fn print_count(
    should_print: bool,
    count: usize,
    label: &str,
    store: Option<&FilesystemScriptStore>,
) {
    if !should_print {
        return;
    }

    if label == "startup trigger" {
        println!("Queued {count} {label}{}.", plural(count));
        return;
    }

    if let Some(store) = store {
        println!(
            "Serving {count} {label}{} from {}.",
            plural(count),
            store.root().display()
        );
    } else {
        println!("Serving {count} {label}{}.", plural(count));
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}
