use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const IPC_PROTOCOL: &str = "baudbound-control-v1";
const MAX_MESSAGE_BYTES: u64 = 4 * 1024;
const STREAM_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceControlCommand {
    Reload,
    Stop,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServiceControlDescriptor {
    pub address: SocketAddr,
    pub protocol: String,
    token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ServiceControlRequest {
    command: ServiceControlCommand,
    protocol: String,
    token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ServiceControlResponse {
    accepted: bool,
    message: String,
    protocol: String,
}

pub struct ServiceControlServer {
    descriptor: ServiceControlDescriptor,
    listener: TcpListener,
}

impl ServiceControlServer {
    pub fn bind() -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .context("failed to bind runner control IPC listener")?;
        listener
            .set_nonblocking(true)
            .context("failed to configure runner control IPC listener")?;
        let address = listener
            .local_addr()
            .context("failed to read runner control IPC address")?;
        let mut token_bytes = [0_u8; 32];
        getrandom::fill(&mut token_bytes)
            .map_err(|error| anyhow!("failed to generate runner control IPC token: {error}"))?;

        Ok(Self {
            descriptor: ServiceControlDescriptor {
                address,
                protocol: IPC_PROTOCOL.to_owned(),
                token: hex_encode(&token_bytes),
            },
            listener,
        })
    }

    pub fn descriptor(&self) -> &ServiceControlDescriptor {
        &self.descriptor
    }

    pub fn poll_command(&self) -> Result<Option<ServiceControlCommand>> {
        let mut selected = None;
        loop {
            let (stream, peer) = match self.listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error).context("runner control IPC accept failed"),
            };
            let command = match self.handle_connection(stream, peer) {
                Ok(command) => command,
                Err(error) => {
                    tracing::warn!(error = %error, peer = %peer, "rejected runner control IPC request");
                    continue;
                }
            };
            if command == ServiceControlCommand::Stop {
                return Ok(Some(command));
            }
            selected = Some(command);
        }
        Ok(selected)
    }

    fn handle_connection(
        &self,
        mut stream: TcpStream,
        peer: SocketAddr,
    ) -> Result<ServiceControlCommand> {
        if !peer.ip().is_loopback() {
            bail!("rejected runner control IPC connection from non-loopback peer {peer}");
        }
        configure_stream(&stream)?;
        let request = read_message::<ServiceControlRequest>(&stream)
            .context("failed to read runner control IPC request")?;

        if request.protocol != IPC_PROTOCOL {
            write_response(&mut stream, false, "unsupported IPC protocol")?;
            bail!("rejected runner control IPC request with unsupported protocol");
        }
        if !constant_time_equal(request.token.as_bytes(), self.descriptor.token.as_bytes()) {
            write_response(&mut stream, false, "authentication failed")?;
            bail!("rejected unauthenticated runner control IPC request");
        }

        write_response(&mut stream, true, "command accepted")?;
        Ok(request.command)
    }
}

pub fn request_service_control(status: &Value, command: ServiceControlCommand) -> Result<()> {
    let descriptor = status
        .get("control")
        .cloned()
        .ok_or_else(|| anyhow!("runner service has not published a live control endpoint"))?;
    let descriptor: ServiceControlDescriptor =
        serde_json::from_value(descriptor).context("runner service control endpoint is invalid")?;
    if descriptor.protocol != IPC_PROTOCOL {
        bail!("runner service uses unsupported IPC protocol");
    }
    if !descriptor.address.ip().is_loopback() {
        bail!("runner service control endpoint is not loopback-only");
    }

    let mut stream = TcpStream::connect_timeout(&descriptor.address, STREAM_TIMEOUT)
        .context("failed to connect to runner service control IPC")?;
    configure_stream(&stream)?;
    write_message(
        &mut stream,
        &ServiceControlRequest {
            command,
            protocol: IPC_PROTOCOL.to_owned(),
            token: descriptor.token,
        },
    )?;
    let response = read_message::<ServiceControlResponse>(&stream)
        .context("failed to read runner control IPC response")?;
    if response.protocol != IPC_PROTOCOL || !response.accepted {
        bail!("runner rejected control command: {}", response.message);
    }
    Ok(())
}

pub fn redact_service_control(status: &mut Value) {
    if let Some(control) = status.get_mut("control").and_then(Value::as_object_mut) {
        control.remove("token");
    }
}

fn configure_stream(stream: &TcpStream) -> Result<()> {
    stream
        .set_read_timeout(Some(STREAM_TIMEOUT))
        .context("failed to configure runner control IPC read timeout")?;
    stream
        .set_write_timeout(Some(STREAM_TIMEOUT))
        .context("failed to configure runner control IPC write timeout")?;
    Ok(())
}

fn read_message<T: for<'de> Deserialize<'de>>(stream: &TcpStream) -> Result<T> {
    let mut bytes = Vec::new();
    BufReader::new(stream)
        .take(MAX_MESSAGE_BYTES + 1)
        .read_until(b'\n', &mut bytes)
        .context("failed to read IPC message")?;
    if bytes.is_empty() {
        bail!("IPC peer closed the connection without a message");
    }
    if bytes.len() as u64 > MAX_MESSAGE_BYTES {
        bail!("IPC message exceeds {MAX_MESSAGE_BYTES} bytes");
    }
    serde_json::from_slice(&bytes).context("IPC message is invalid")
}

fn write_message<T: Serialize>(stream: &mut TcpStream, message: &T) -> Result<()> {
    serde_json::to_writer(&mut *stream, message).context("failed to serialize IPC message")?;
    stream
        .write_all(b"\n")
        .context("failed to write IPC message")?;
    stream.flush().context("failed to flush IPC message")
}

fn write_response(stream: &mut TcpStream, accepted: bool, message: &str) -> Result<()> {
    write_message(
        stream,
        &ServiceControlResponse {
            accepted,
            message: message.to_owned(),
            protocol: IPC_PROTOCOL.to_owned(),
        },
    )
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;
