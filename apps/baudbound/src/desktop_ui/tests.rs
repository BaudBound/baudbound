use std::{
    fs,
    io::{Cursor, Write},
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};
use tauri::{WebviewWindowBuilder, ipc::InvokeBody, test, webview::InvokeRequest};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::*;

#[test]
fn tauri_bridge_completes_the_primary_desktop_workflow() {
    let temporary_directory = tempfile::tempdir().expect("temporary directory should be created");
    let runner_home = temporary_directory.path().join("runner");
    let config_path = runner_home.join("config.toml");
    RunnerConfig::write_template(&config_path).expect("runner config should be initialized");
    let runner_config =
        RunnerConfig::load_or_init(&config_path).expect("runner config should load");
    let websocket_registry = Arc::new(WebSocketConnectionRegistry::new());
    let store = SqliteRunnerStore::open(runner_home.join("runner.sqlite3"))
        .expect("SQLite runner store should open");
    let state = DesktopUiState {
        background_options: Mutex::new(desktop_background_options(
            &runner_config,
            Arc::clone(&websocket_registry),
            config_path.clone(),
        )),
        background_runner: DesktopRunnerSupervisor::default(),
        config_path: config_path.clone(),
        login_startup_registered: Mutex::new(None),
        runner_config: Mutex::new(runner_config.clone()),
        core: Arc::new(Mutex::new(build_runner_core(
            &runner_config,
            Arc::clone(&websocket_registry),
        ))),
        store,
        websocket_registry,
        operation_lock: Arc::new(Mutex::new(())),
    };
    let app = test::mock_builder()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("BaudBound")
                .args(["ui", "--autostart"])
                .build(),
        )
        .manage(coordinate_picker::CoordinatePickerState::default())
        .manage(state)
        .invoke_handler(desktop_command_handler!())
        .build(test::mock_context(test::noop_assets()))
        .expect("mock Tauri app should build");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview should build");

    let initial = invoke(&webview, "dashboard_state", json!({}));
    assert_eq!(initial["runner"]["total_script_count"], 0);

    let package_path = temporary_directory.path().join("desktop-workflow.bbs");
    fs::write(&package_path, create_test_package()).expect("test package should be written");
    let imported = invoke(
        &webview,
        "import_script_package",
        json!({"packagePath": package_path}),
    );
    assert_eq!(imported["dashboard"]["runner"]["total_script_count"], 1);
    assert!(
        imported["message"]
            .as_str()
            .is_some_and(|value| value.starts_with("Imported"))
    );

    let approved = invoke(
        &webview,
        "approve_script",
        json!({"reference": "desktop-workflow"}),
    );
    assert_eq!(
        approved["dashboard"]["runner"]["scripts"][0]["approval_status"]["state"],
        "current"
    );

    let run = invoke(
        &webview,
        "run_script",
        json!({"reference": "desktop-workflow"}),
    );
    assert_eq!(run["dashboard"]["recent_runs"][0]["status"], "completed");
    assert_eq!(
        run["dashboard"]["recent_runs"][0]["logs"][1]["message"],
        "desktop bridge"
    );
    assert!(
        run["dashboard"]["recent_runs"][0]["logs"][1]["timestamp_unix_ms"]
            .as_u64()
            .is_some_and(|value| value > 0)
    );

    let disabled = invoke(
        &webview,
        "set_script_enabled",
        json!({"reference": "desktop-workflow", "enabled": false}),
    );
    assert_eq!(disabled["dashboard"]["runner"]["enabled_script_count"], 0);
    invoke(
        &webview,
        "set_script_enabled",
        json!({"reference": "desktop-workflow", "enabled": true}),
    );

    let config = invoke(&webview, "read_runner_config", json!({}));
    assert_eq!(config["config"]["display"]["time_format"], "24-hour");
    assert_eq!(config["config"]["updates"]["automatic_checks"], true);
    assert_eq!(config["config"]["desktop"]["launch_at_login"], false);
    let mut contents = config["contents"]
        .as_str()
        .expect("config contents should be returned")
        .to_owned();
    contents = contents.replace(
        "run_history_max_records = 10000",
        "run_history_max_records = 250",
    );
    invoke(
        &webview,
        "save_runner_config",
        json!({"contents": contents, "restartBackground": false}),
    );
    let saved = invoke(&webview, "read_runner_config", json!({}));
    assert_eq!(saved["config"]["runner"]["run_history_max_records"], 250);

    let reset = invoke(
        &webview,
        "reset_runner_config",
        json!({"restartBackground": false}),
    );
    assert_eq!(reset["message"], "Reset runner config to defaults.");
    let reset_config = invoke(&webview, "read_runner_config", json!({}));
    assert_eq!(
        reset_config["config"]["runner"]["run_history_max_records"],
        10_000
    );
    assert!(
        reset_config["contents"]
            .as_str()
            .is_some_and(|contents| contents == RunnerConfig::template_toml())
    );

    let serial_ports = invoke(&webview, "scan_serial_ports", json!({}));
    assert!(serial_ports.is_array());

    let monitor_discovery = invoke(&webview, "discover_monitors", json!({}));
    assert_eq!(monitor_discovery["supported"], cfg!(windows));
    if cfg!(windows) {
        assert!(
            monitor_discovery["monitors"]
                .as_array()
                .is_some_and(|monitors| !monitors.is_empty())
        );
        assert!(monitor_discovery["virtual_bounds"].is_object());
    } else {
        assert!(
            monitor_discovery["monitors"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
        assert!(monitor_discovery["unavailable_reason"].is_string());
    }

    #[cfg(windows)]
    {
        let picker = invoke(&webview, "start_coordinate_picker", json!({}));
        assert!(
            picker["monitor_count"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        let picker_session = picker["session_id"]
            .as_str()
            .expect("picker session ID should be returned");
        invoke(
            &webview,
            "select_coordinate_picker",
            json!({"sessionId": picker_session}),
        );

        let cancellable_picker = invoke(&webview, "start_coordinate_picker", json!({}));
        let cancellable_session = cancellable_picker["session_id"]
            .as_str()
            .expect("second picker session ID should be returned");
        invoke(
            &webview,
            "cancel_coordinate_picker",
            json!({"sessionId": cancellable_session}),
        );
    }

    let started = invoke(&webview, "start_background_runner", json!({}));
    assert_eq!(started["dashboard"]["desktop_background"]["running"], true);
    invoke(&webview, "stop_background_runner", json!({}));
    let stopped = invoke(&webview, "prepare_for_update", json!({}));
    assert_eq!(stopped["dashboard"]["desktop_background"]["running"], false);

    let removed = invoke(
        &webview,
        "remove_script",
        json!({"reference": "desktop-workflow"}),
    );
    assert_eq!(removed["dashboard"]["runner"]["total_script_count"], 0);
}

#[test]
fn only_runtime_owned_config_requires_a_background_restart() {
    let previous = RunnerConfig::default();

    let mut desktop = previous.clone();
    desktop.desktop.keep_running_on_close = false;
    assert!(!runner_runtime_config_changed(&previous, &desktop));

    let mut display = previous.clone();
    display.display.time_format = TimeFormat::TwelveHour;
    assert!(!runner_runtime_config_changed(&previous, &display));

    let mut updates = previous.clone();
    updates.updates.check_interval_hours = 6;
    assert!(!runner_runtime_config_changed(&previous, &updates));

    let mut runner = previous.clone();
    runner.runner.trigger_reload_seconds = 5;
    assert!(runner_runtime_config_changed(&previous, &runner));
}

fn invoke(webview: &tauri::WebviewWindow<test::MockRuntime>, command: &str, body: Value) -> Value {
    test::get_ipc_response(
        webview,
        InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .expect("test URL should parse"),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: test::INVOKE_KEY.to_owned(),
        },
    )
    .unwrap_or_else(|error| panic!("Tauri command {command:?} failed: {error}"))
    .deserialize::<Value>()
    .unwrap_or_else(|error| panic!("Tauri command {command:?} returned invalid JSON: {error}"))
}

fn create_test_package() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (path, content) in [
        (
            "manifest.json",
            r#"{
                "format_version": 1,
                "script_language_version": 1,
                "id": "desktop-workflow",
                "name": "Desktop Workflow",
                "created_with": "BaudBound Tauri Test",
                "created_at": "2026-01-01T00:00:00.000Z",
                "minimum_runner_version": "0.1.0"
            }"#,
        ),
        (
            "program.json",
            r#"{
                "entry": {
                    "trigger": {
                        "id": "n-manual",
                        "action_type": "trigger.manual",
                        "type": "manual",
                        "config": {},
                        "runtime_outputs": []
                    },
                    "triggers": [],
                    "program": {
                        "type": "block",
                        "execution_model": "directed_graph",
                        "runtime_context": {
                            "expression_reference": "{{node-id.data_name}}",
                            "template_reference": "{{node-id.data_name}}",
                            "variables": [],
                            "built_in_variables": {"syntax": "{{variable_name}}", "variables": []},
                            "node_outputs": []
                        },
                        "steps": [{
                            "id": "n-log",
                            "action_type": "action.log",
                            "type": "action",
                            "action": "log",
                            "config": {"level": "info", "message": "desktop bridge"},
                            "runtime_outputs": []
                        }],
                        "edges": [{
                            "execution_order": 0,
                            "source": "n-manual",
                            "source_handle": "out",
                            "target": "n-log",
                            "target_handle": "input"
                        }]
                    }
                }
            }"#,
        ),
        (
            "permissions.json",
            r#"{"declared_permissions": ["log"], "risk_level": "low"}"#,
        ),
        (
            "capabilities.json",
            r#"{"required_capabilities": ["action.log", "trigger.manual"], "target_runtime": "Generic Desktop"}"#,
        ),
    ] {
        writer
            .start_file(path, options)
            .expect("test package entry should start");
        writer
            .write_all(content.as_bytes())
            .expect("test package entry should write");
    }
    writer
        .finish()
        .expect("test package should finish")
        .into_inner()
}
