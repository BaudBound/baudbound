use std::{thread, time::Duration};

use serde_json::json;

use super::*;

#[test]
fn authenticated_client_sends_reload_command() {
    let server = ServiceControlServer::bind().expect("IPC server should bind");
    let status = json!({ "control": server.descriptor() });
    let client = thread::spawn(move || {
        request_service_control(&status, ServiceControlCommand::Reload)
            .expect("reload request should succeed");
    });

    let command = wait_for_command(&server);
    client.join().expect("IPC client should finish");
    assert_eq!(command, ServiceControlCommand::Reload);
}

#[test]
fn unauthenticated_client_is_rejected() {
    let server = ServiceControlServer::bind().expect("IPC server should bind");
    let status = json!({
        "control": {
            "address": server.descriptor().address,
            "protocol": IPC_PROTOCOL,
            "token": "invalid"
        }
    });
    let client = thread::spawn(move || {
        assert!(request_service_control(&status, ServiceControlCommand::Stop).is_err());
    });

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !client.is_finished() && std::time::Instant::now() < deadline {
        assert_eq!(
            server
                .poll_command()
                .expect("IPC server should keep running"),
            None
        );
        thread::sleep(Duration::from_millis(10));
    }
    client.join().expect("IPC client should finish");
    assert_eq!(
        server
            .poll_command()
            .expect("IPC server should keep running"),
        None
    );
}

fn wait_for_command(server: &ServiceControlServer) -> ServiceControlCommand {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match server.poll_command().expect("IPC server should poll") {
            Some(command) => return command,
            None if std::time::Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            None => panic!("timed out waiting for IPC command"),
        }
    }
}
