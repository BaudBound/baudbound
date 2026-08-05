use std::{
    io::Write,
    net::TcpStream,
    thread,
    time::{Duration, Instant},
};

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

#[test]
fn incomplete_client_does_not_stall_command_polling_or_other_clients() {
    let server = ServiceControlServer::bind().expect("IPC server should bind");
    let incomplete =
        TcpStream::connect(server.descriptor().address).expect("incomplete client should connect");
    let status = json!({ "control": server.descriptor() });
    let client = thread::spawn(move || {
        request_service_control(&status, ServiceControlCommand::Reload)
            .expect("authenticated client should not wait for incomplete peer");
    });

    let started = Instant::now();
    let command = wait_for_command(&server);
    assert!(
        started.elapsed() < STREAM_TIMEOUT,
        "service polling waited for the incomplete client's read timeout"
    );
    assert_eq!(command, ServiceControlCommand::Reload);
    client.join().expect("IPC client should finish");
    drop(incomplete);
}

#[test]
fn dropping_server_with_incomplete_client_is_bounded() {
    let server = ServiceControlServer::bind().expect("IPC server should bind");
    let incomplete =
        TcpStream::connect(server.descriptor().address).expect("incomplete client should connect");
    thread::sleep(Duration::from_millis(25));

    let started = Instant::now();
    drop(server);
    assert!(
        started.elapsed() < STREAM_TIMEOUT + Duration::from_millis(250),
        "IPC shutdown exceeded its stream deadline"
    );
    drop(incomplete);
}

#[test]
fn incomplete_client_flood_does_not_block_service_polling_or_shutdown() {
    let server = ServiceControlServer::bind().expect("IPC server should bind");
    let incomplete = (0..MAX_UNAUTHENTICATED_CONNECTIONS)
        .map(|_| {
            TcpStream::connect(server.descriptor().address)
                .expect("incomplete client should connect")
        })
        .collect::<Vec<_>>();
    thread::sleep(Duration::from_millis(25));

    let polling_started = Instant::now();
    for _ in 0..100 {
        assert_eq!(
            server
                .poll_command()
                .expect("IPC service should remain responsive"),
            None
        );
    }
    assert!(
        polling_started.elapsed() < Duration::from_millis(100),
        "incomplete clients must not run on the service polling path"
    );

    let shutdown_started = Instant::now();
    drop(server);
    assert!(
        shutdown_started.elapsed() < STREAM_TIMEOUT + Duration::from_millis(250),
        "IPC shutdown must stay bounded under a full incomplete-client load"
    );
    drop(incomplete);
}

#[test]
fn authenticated_reload_remains_bounded_during_an_incomplete_client_flood() {
    let server = ServiceControlServer::bind().expect("IPC server should bind");
    let incomplete = (0..MAX_UNAUTHENTICATED_CONNECTIONS)
        .map(|_| {
            TcpStream::connect(server.descriptor().address)
                .expect("incomplete client should connect")
        })
        .collect::<Vec<_>>();
    thread::sleep(Duration::from_millis(25));

    let status = json!({ "control": server.descriptor() });
    let client = thread::spawn(move || {
        request_service_control(&status, ServiceControlCommand::Reload)
            .expect("authenticated reload should complete after stale reads expire");
    });
    let started = Instant::now();
    let command = wait_for_command(&server);

    assert_eq!(command, ServiceControlCommand::Reload);
    assert!(
        started.elapsed() < STREAM_TIMEOUT + Duration::from_secs(1),
        "authenticated control must recover within the bounded unauthenticated read deadline"
    );
    client.join().expect("IPC client should finish");
    drop(incomplete);
}

#[test]
fn malformed_client_flood_does_not_delay_polling_or_authenticated_reload() {
    const CLIENTS: usize = MAX_UNAUTHENTICATED_CONNECTIONS * 4;

    let server = ServiceControlServer::bind().expect("IPC server should bind");
    let address = server.descriptor().address;
    let clients = (0..CLIENTS)
        .map(|index| {
            thread::spawn(move || {
                let mut stream = TcpStream::connect_timeout(&address, STREAM_TIMEOUT)
                    .expect("malformed test client should connect");
                stream
                    .set_write_timeout(Some(STREAM_TIMEOUT))
                    .expect("malformed test client timeout should configure");
                let request = match index % 3 {
                    0 => b"{not-json}\n".to_vec(),
                    1 => {
                        let mut oversized =
                            vec![b'x'; usize::try_from(MAX_MESSAGE_BYTES).unwrap() + 1];
                        oversized.push(b'\n');
                        oversized
                    }
                    _ => serde_json::to_vec(&ServiceControlRequest {
                        command: ServiceControlCommand::Reload,
                        protocol: IPC_PROTOCOL.to_owned(),
                        token: "invalid".to_owned(),
                    })
                    .expect("invalid authenticated request should serialize"),
                };
                stream
                    .write_all(&request)
                    .expect("malformed test request should write");
            })
        })
        .collect::<Vec<_>>();

    let polling_started = Instant::now();
    for _ in 0..1_000 {
        assert_eq!(
            server
                .poll_command()
                .expect("IPC service should remain responsive"),
            None
        );
    }
    assert!(
        polling_started.elapsed() < Duration::from_millis(100),
        "malformed clients must not execute on the service polling path"
    );
    for client in clients {
        client.join().expect("malformed test client should finish");
    }

    let status = json!({ "control": server.descriptor() });
    let authenticated = thread::spawn(move || {
        request_service_control(&status, ServiceControlCommand::Reload)
            .expect("authenticated reload should recover after malformed traffic");
    });
    let started = Instant::now();
    assert_eq!(wait_for_command(&server), ServiceControlCommand::Reload);
    assert!(
        started.elapsed() < STREAM_TIMEOUT + Duration::from_secs(1),
        "authenticated reload must remain bounded after malformed client pressure"
    );
    authenticated
        .join()
        .expect("authenticated client should finish");
}

#[test]
fn stale_descriptor_cannot_control_a_replacement_server() {
    let old_server = ServiceControlServer::bind().expect("old IPC server should bind");
    let stale_status = json!({ "control": old_server.descriptor() });
    drop(old_server);

    let replacement = ServiceControlServer::bind().expect("replacement IPC server should bind");
    assert!(
        request_service_control(&stale_status, ServiceControlCommand::Stop).is_err(),
        "stale endpoint metadata must not authenticate to another server"
    );
    assert_eq!(
        replacement
            .poll_command()
            .expect("replacement IPC server should keep running"),
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
