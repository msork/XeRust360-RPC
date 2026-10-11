#[cfg(windows)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::path::PathBuf;
use std::{
    io::{Read, Write},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

use crate::presence::PresencePayload;

const HANDSHAKE_OPCODE: u32 = 0;
const FRAME_OPCODE: u32 = 1;
const MAX_PACKET_BYTES: usize = 1_048_576;

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscordError {
    InvalidApplicationId,
    NotRunning,
    Io(String),
    Protocol(String),
}

impl std::fmt::Display for DiscordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidApplicationId => write!(f, "Discord application ID must be a number"),
            Self::NotRunning => write!(f, "Discord desktop client is not available"),
            Self::Io(_) => write!(f, "Discord local IPC connection failed"),
            Self::Protocol(message) => write!(f, "Discord RPC protocol error: {message}"),
        }
    }
}

impl std::error::Error for DiscordError {}

pub struct DiscordRpcClient {
    stream: Box<dyn ReadWrite + Send>,
    next_nonce: u64,
    process_id: u32,
}

impl DiscordRpcClient {
    /// Connects to Discord's local IPC server and performs the official RPC handshake.
    pub fn connect(application_id: &str) -> Result<Self, DiscordError> {
        if application_id.is_empty() || !application_id.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(DiscordError::InvalidApplicationId);
        }
        let stream = connect_local_ipc()?;
        Self::handshake(stream, application_id)
    }

    fn handshake(
        mut stream: Box<dyn ReadWrite + Send>,
        application_id: &str,
    ) -> Result<Self, DiscordError> {
        let handshake = json!({ "v": 1, "client_id": application_id });
        write_packet(
            &mut *stream,
            HANDSHAKE_OPCODE,
            handshake.to_string().as_bytes(),
        )?;
        let (opcode, response) = read_packet(&mut *stream)?;
        if opcode != FRAME_OPCODE {
            return Err(DiscordError::Protocol(
                "unexpected handshake frame".to_owned(),
            ));
        }
        let response: Value = serde_json::from_slice(&response)
            .map_err(|_| DiscordError::Protocol("invalid handshake response".to_owned()))?;
        if response.get("evt").and_then(Value::as_str) != Some("READY") {
            return Err(DiscordError::Protocol(
                "Discord did not complete the handshake".to_owned(),
            ));
        }
        Ok(Self {
            stream,
            next_nonce: 1,
            process_id: std::process::id(),
        })
    }

    pub fn set_presence(&mut self, presence: &PresencePayload) -> Result<(), DiscordError> {
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(1).max(1);
        let activity = activity_object(presence);
        let packet = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": self.process_id,
                "activity": activity,
            },
            "nonce": nonce.to_string(),
        });
        write_packet(
            &mut *self.stream,
            FRAME_OPCODE,
            packet.to_string().as_bytes(),
        )
    }

    pub fn clear_presence(&mut self) -> Result<(), DiscordError> {
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(1).max(1);
        let packet = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": self.process_id, "activity": null },
            "nonce": nonce.to_string(),
        });
        write_packet(
            &mut *self.stream,
            FRAME_OPCODE,
            packet.to_string().as_bytes(),
        )
    }
}

fn activity_object(presence: &PresencePayload) -> Value {
    let mut activity = json!({
        "type": 0,
        "details": presence.details,
        "assets": {
            "large_image": presence.large_image,
            "small_image": presence.small_image,
        },
    });
    if !presence.state.is_empty() {
        activity["state"] = Value::String(presence.state.clone());
    }
    if let Some(elapsed) = presence.elapsed {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let start = now.saturating_sub(elapsed.as_secs());
        activity["timestamps"] = json!({ "start": start });
    }
    activity
}

fn write_packet(
    stream: &mut (impl Write + ?Sized),
    opcode: u32,
    payload: &[u8],
) -> Result<(), DiscordError> {
    if payload.len() > MAX_PACKET_BYTES {
        return Err(DiscordError::Protocol(
            "packet exceeds one megabyte".to_owned(),
        ));
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| DiscordError::Protocol("packet is too large".to_owned()))?;
    stream
        .write_all(&opcode.to_le_bytes())
        .and_then(|()| stream.write_all(&length.to_le_bytes()))
        .and_then(|()| stream.write_all(payload))
        .and_then(|()| stream.flush())
        .map_err(|_| DiscordError::Io("write failed".to_owned()))
}

fn read_packet(stream: &mut (impl Read + ?Sized)) -> Result<(u32, Vec<u8>), DiscordError> {
    let mut header = [0u8; 8];
    stream
        .read_exact(&mut header)
        .map_err(|_| DiscordError::Io("read failed".to_owned()))?;
    let opcode = u32::from_le_bytes(header[..4].try_into().expect("fixed header"));
    let length = u32::from_le_bytes(header[4..].try_into().expect("fixed header")) as usize;
    if length > MAX_PACKET_BYTES {
        return Err(DiscordError::Protocol(
            "packet exceeds one megabyte".to_owned(),
        ));
    }
    let mut payload = vec![0; length];
    stream
        .read_exact(&mut payload)
        .map_err(|_| DiscordError::Io("read failed".to_owned()))?;
    Ok((opcode, payload))
}

#[cfg(unix)]
fn connect_local_ipc() -> Result<Box<dyn ReadWrite + Send>, DiscordError> {
    use std::os::unix::net::UnixStream;

    let mut directories = Vec::new();
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        directories.push(PathBuf::from(runtime_dir));
    }
    directories.push(std::env::temp_dir());
    directories.push(PathBuf::from("/tmp"));
    directories.dedup();
    for directory in directories {
        for index in 0..10 {
            let path = directory.join(format!("discord-ipc-{index}"));
            if let Ok(stream) = UnixStream::connect(path) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                return Ok(Box::new(stream));
            }
        }
    }
    Err(DiscordError::NotRunning)
}

#[cfg(windows)]
fn connect_local_ipc() -> Result<Box<dyn ReadWrite + Send>, DiscordError> {
    for index in 0..10 {
        let path = format!(r"\\?\pipe\discord-ipc-{index}");
        if let Ok(file) = OpenOptions::new().read(true).write(true).open(path) {
            return Ok(Box::new(file));
        }
    }
    Err(DiscordError::NotRunning)
}

#[cfg(not(any(unix, windows)))]
fn connect_local_ipc() -> Result<Box<dyn ReadWrite + Send>, DiscordError> {
    Err(DiscordError::NotRunning)
}

#[cfg(test)]
mod tests {
    use std::{
        io::Cursor,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use super::*;
    use crate::{domain::StateToggles, mock::sample_snapshot, presence::format_presence};

    struct MemoryStream {
        input: Cursor<Vec<u8>>,
        output: Arc<Mutex<Vec<u8>>>,
    }

    impl Read for MemoryStream {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buffer)
        }
    }

    impl Write for MemoryStream {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.output.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn frame(opcode: u32, payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend(opcode.to_le_bytes());
        frame.extend((payload.len() as u32).to_le_bytes());
        frame.extend(payload);
        frame
    }

    #[test]
    fn constructs_rpc_activity_with_elapsed_timestamp_and_assets() {
        let presence = format_presence(
            &sample_snapshot(),
            StateToggles::default(),
            Some(Duration::from_secs(20)),
        )
        .unwrap();
        let activity = activity_object(&presence);
        assert_eq!(activity["type"], 0);
        assert_eq!(activity["details"], "Halo 3");
        assert_eq!(activity["state"], presence.state);
        assert_eq!(activity["assets"]["large_image"], "halo-3");
        assert!(activity["timestamps"]["start"].as_u64().is_some());
    }

    #[test]
    fn rejects_non_numeric_application_ids() {
        assert!(matches!(
            DiscordRpcClient::connect("not-a-token"),
            Err(DiscordError::InvalidApplicationId)
        ));
    }

    #[test]
    fn completes_handshake_and_sends_set_activity_frame() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let stream = MemoryStream {
            input: Cursor::new(frame(FRAME_OPCODE, br#"{"evt":"READY"}"#)),
            output: Arc::clone(&output),
        };
        let mut client = DiscordRpcClient::handshake(Box::new(stream), "12345").unwrap();

        let presence = format_presence(
            &sample_snapshot(),
            StateToggles::default(),
            Some(Duration::from_secs(5)),
        )
        .unwrap();
        client.set_presence(&presence).unwrap();

        let output = output.lock().unwrap();
        let mut captured = &output[..];
        let (opcode, handshake) = read_packet(&mut captured).unwrap();
        assert_eq!(opcode, HANDSHAKE_OPCODE);
        let handshake: Value = serde_json::from_slice(&handshake).unwrap();
        assert_eq!(handshake["client_id"], "12345");
        let (opcode, packet) = read_packet(&mut captured).unwrap();
        assert_eq!(opcode, FRAME_OPCODE);
        let packet: Value = serde_json::from_slice(&packet).unwrap();
        assert_eq!(packet["cmd"], "SET_ACTIVITY");
        assert_eq!(packet["args"]["activity"]["details"], "Halo 3");
        assert!(packet.get("nonce").is_some());
    }

    #[test]
    fn clear_sends_null_activity() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let stream = MemoryStream {
            input: Cursor::new(frame(FRAME_OPCODE, br#"{"evt":"READY"}"#)),
            output: Arc::clone(&output),
        };
        let mut client = DiscordRpcClient::handshake(Box::new(stream), "12345").unwrap();
        client.clear_presence().unwrap();
        let output = output.lock().unwrap();
        let mut captured = &output[..];
        let _ = read_packet(&mut captured).unwrap();
        let (_, packet) = read_packet(&mut captured).unwrap();
        let packet: Value = serde_json::from_slice(&packet).unwrap();
        assert_eq!(packet["cmd"], "SET_ACTIVITY");
        assert!(packet["args"]["activity"].is_null());
    }
}
