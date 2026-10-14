//! Versioned, authenticated, read-only LAN companion protocol.
//!
//! This module implements protocol primitives only. It does not load, build,
//! install, or deploy an Xbox executable.

use std::{
    fmt,
    io::{self, Read, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    sync::mpsc::TryRecvError,
    time::Instant,
};

use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::domain::{AchievementProgress, Gamerscore, TelemetrySnapshot, Temperatures};

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_FRAME_BYTES: usize = 4096;
const AUTH_DOMAIN: &[u8] = b"XeRust360-RPC companion auth v1";
const TELEMETRY_DOMAIN: &[u8] = b"XeRust360-RPC companion telemetry v1";

#[derive(Clone, PartialEq, Eq)]
pub struct SharedKey([u8; 32]);

impl SharedKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn generate() -> Result<Self, ProtocolError> {
        let mut bytes = [0; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| ProtocolError::RandomUnavailable)?;
        Ok(Self(bytes))
    }

    pub fn from_hex(text: &str) -> Result<Self, ProtocolError> {
        parse_hex_32(text)
            .map(Self)
            .ok_or(ProtocolError::InvalidTelemetry("shared key"))
    }

    pub fn to_hex(&self) -> String {
        hex(&self.0)
    }
}

impl fmt::Debug for SharedKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SharedKey([REDACTED])")
    }
}

impl Drop for SharedKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Challenge([u8; 32]);

impl Challenge {
    pub fn generate() -> Result<Self, ProtocolError> {
        let mut bytes = [0; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| ProtocolError::RandomUnavailable)?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn message_frame(&self) -> Result<Vec<u8>, ProtocolError> {
        let message = ChallengeMessage {
            version: PROTOCOL_VERSION,
            message_type: "challenge".to_owned(),
            challenge: hex(&self.0),
        };
        encode_frame(&serde_json::to_vec(&message).map_err(|_| ProtocolError::InvalidJson)?)
    }

    fn from_message(payload: &[u8]) -> Result<Self, ProtocolError> {
        let message: ChallengeMessage = parse_canonical(payload)?;
        if message.version != PROTOCOL_VERSION {
            return Err(ProtocolError::WrongVersion);
        }
        if message.message_type != "challenge" {
            return Err(ProtocolError::WrongMessageType);
        }
        Ok(Self(
            parse_hex_32(&message.challenge).ok_or(ProtocolError::InvalidMac)?,
        ))
    }
}

impl fmt::Debug for Challenge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Challenge([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidFrameLength,
    OversizedFrame,
    InvalidJson,
    NonCanonicalJson,
    WrongVersion,
    WrongMessageType,
    InvalidMac,
    InvalidSequence,
    InvalidTelemetry(&'static str),
    RandomUnavailable,
    Io(io::ErrorKind),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrameLength => formatter.write_str("invalid companion frame length"),
            Self::OversizedFrame => formatter.write_str("companion frame exceeds size limit"),
            Self::InvalidJson => formatter.write_str("invalid companion message"),
            Self::NonCanonicalJson => formatter.write_str("companion message is not canonical"),
            Self::WrongVersion => formatter.write_str("unsupported companion protocol version"),
            Self::WrongMessageType => formatter.write_str("unexpected companion message type"),
            Self::InvalidMac => formatter.write_str("companion authentication failed"),
            Self::InvalidSequence => formatter.write_str("companion sequence is stale or repeated"),
            Self::InvalidTelemetry(field) => write!(formatter, "invalid companion field: {field}"),
            Self::RandomUnavailable => formatter.write_str("secure random source unavailable"),
            Self::Io(_) => formatter.write_str("companion connection failed"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthMessage {
    version: u8,
    message_type: String,
    mac: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeMessage {
    version: u8,
    message_type: String,
    challenge: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnsignedTelemetry {
    version: u8,
    message_type: String,
    sequence: u64,
    title_id: Option<String>,
    gamertag: Option<String>,
    achievements_unlocked: Option<u32>,
    achievements_total: Option<u32>,
    game_gamerscore: Option<u32>,
    total_gamerscore: Option<u32>,
    cpu_celsius: Option<f32>,
    gpu_celsius: Option<f32>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTelemetry {
    version: u8,
    message_type: String,
    sequence: u64,
    title_id: Option<String>,
    gamertag: Option<String>,
    achievements_unlocked: Option<u32>,
    achievements_total: Option<u32>,
    game_gamerscore: Option<u32>,
    total_gamerscore: Option<u32>,
    cpu_celsius: Option<f32>,
    gpu_celsius: Option<f32>,
    mac: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionTelemetry {
    pub title_id: Option<String>,
    pub gamertag: Option<String>,
    pub achievements_unlocked: Option<u32>,
    pub achievements_total: Option<u32>,
    pub game_gamerscore: Option<u32>,
    pub total_gamerscore: Option<u32>,
    pub cpu_celsius: Option<f32>,
    pub gpu_celsius: Option<f32>,
}

impl CompanionTelemetry {
    pub fn into_snapshot(self) -> Result<TelemetrySnapshot, ProtocolError> {
        validate_fields(&self)?;
        Ok(TelemetrySnapshot {
            game: crate::domain::GameIdentity {
                title_id: self.title_id,
                name: None,
                ..Default::default()
            },
            gamertag: self.gamertag,
            achievements: AchievementProgress {
                unlocked: self.achievements_unlocked,
                total: self.achievements_total,
            },
            gamerscore: Gamerscore {
                game: self.game_gamerscore,
                total: self.total_gamerscore,
            },
            temperatures: Temperatures {
                cpu_celsius: self.cpu_celsius,
                gpu_celsius: self.gpu_celsius,
            },
        })
    }
}

pub fn encode_frame(payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if payload.is_empty() {
        return Err(ProtocolError::InvalidFrameLength);
    }
    if payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::OversizedFrame);
    }
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

pub fn decode_frame(frame: &[u8]) -> Result<&[u8], ProtocolError> {
    if frame.len() < 4 {
        return Err(ProtocolError::InvalidFrameLength);
    }
    let length = u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize;
    if length == 0 {
        return Err(ProtocolError::InvalidFrameLength);
    }
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::OversizedFrame);
    }
    if frame.len() != 4 + length {
        return Err(ProtocolError::InvalidFrameLength);
    }
    Ok(&frame[4..])
}

pub fn encode_authentication(key: &SharedKey, challenge: &Challenge) -> Vec<u8> {
    let mut input = Vec::with_capacity(AUTH_DOMAIN.len() + challenge.0.len());
    input.extend_from_slice(AUTH_DOMAIN);
    input.extend_from_slice(&challenge.0);
    let message = AuthMessage {
        version: PROTOCOL_VERSION,
        message_type: "authenticate".to_owned(),
        mac: hex(&hmac_sha256(&key.0, &input)),
    };
    serde_json::to_vec(&message).expect("auth message serialization is infallible")
}

pub fn verify_authentication(
    key: &SharedKey,
    challenge: &Challenge,
    payload: &[u8],
) -> Result<(), ProtocolError> {
    let message: AuthMessage = parse_canonical(payload)?;
    if message.version != PROTOCOL_VERSION {
        return Err(ProtocolError::WrongVersion);
    }
    if message.message_type != "authenticate" {
        return Err(ProtocolError::WrongMessageType);
    }
    let supplied = parse_hex_32(&message.mac).ok_or(ProtocolError::InvalidMac)?;
    let mut input = Vec::with_capacity(AUTH_DOMAIN.len() + challenge.0.len());
    input.extend_from_slice(AUTH_DOMAIN);
    input.extend_from_slice(&challenge.0);
    if !bool::from(hmac_sha256(&key.0, &input).ct_eq(&supplied)) {
        return Err(ProtocolError::InvalidMac);
    }
    Ok(())
}

pub fn encode_telemetry(
    key: &SharedKey,
    challenge: &Challenge,
    sequence: u64,
    telemetry: &CompanionTelemetry,
) -> Result<Vec<u8>, ProtocolError> {
    validate_fields(telemetry)?;
    let unsigned = to_unsigned(sequence, telemetry.clone());
    let unsigned_bytes =
        serde_json::to_vec(&unsigned).expect("telemetry serialization is infallible");
    let mac = telemetry_mac(key, challenge, sequence, &unsigned_bytes);
    let wire = WireTelemetry {
        version: unsigned.version,
        message_type: unsigned.message_type.clone(),
        sequence,
        title_id: unsigned.title_id.clone(),
        gamertag: unsigned.gamertag.clone(),
        achievements_unlocked: unsigned.achievements_unlocked,
        achievements_total: unsigned.achievements_total,
        game_gamerscore: unsigned.game_gamerscore,
        total_gamerscore: unsigned.total_gamerscore,
        cpu_celsius: unsigned.cpu_celsius,
        gpu_celsius: unsigned.gpu_celsius,
        mac: hex(&mac),
    };
    let payload = serde_json::to_vec(&wire).expect("telemetry serialization is infallible");
    encode_frame(&payload)
}

#[derive(Default)]
pub struct AuthenticatedSession {
    last_sequence: Option<u64>,
}

impl AuthenticatedSession {
    pub fn verify_next(
        &mut self,
        key: &SharedKey,
        challenge: &Challenge,
        frame: &[u8],
    ) -> Result<(u64, TelemetrySnapshot), ProtocolError> {
        let payload = decode_frame(frame)?;
        let wire: WireTelemetry = parse_canonical(payload)?;
        if wire.version != PROTOCOL_VERSION {
            return Err(ProtocolError::WrongVersion);
        }
        if wire.message_type != "telemetry" {
            return Err(ProtocolError::WrongMessageType);
        }
        if self.last_sequence.is_some_and(|last| wire.sequence <= last) {
            return Err(ProtocolError::InvalidSequence);
        }
        let unsigned = unsigned_from_wire(&wire);
        let unsigned_bytes =
            serde_json::to_vec(&unsigned).expect("telemetry serialization is infallible");
        let supplied = parse_hex_32(&wire.mac).ok_or(ProtocolError::InvalidMac)?;
        let expected = telemetry_mac(key, challenge, wire.sequence, &unsigned_bytes);
        if !bool::from(expected.ct_eq(&supplied)) {
            return Err(ProtocolError::InvalidMac);
        }
        let telemetry = CompanionTelemetry {
            title_id: unsigned.title_id,
            gamertag: unsigned.gamertag,
            achievements_unlocked: unsigned.achievements_unlocked,
            achievements_total: unsigned.achievements_total,
            game_gamerscore: unsigned.game_gamerscore,
            total_gamerscore: unsigned.total_gamerscore,
            cpu_celsius: unsigned.cpu_celsius,
            gpu_celsius: unsigned.gpu_celsius,
        };
        let snapshot = telemetry.into_snapshot()?;
        self.last_sequence = Some(wire.sequence);
        Ok((wire.sequence, snapshot))
    }
}

fn to_unsigned(sequence: u64, telemetry: CompanionTelemetry) -> UnsignedTelemetry {
    UnsignedTelemetry {
        version: PROTOCOL_VERSION,
        message_type: "telemetry".to_owned(),
        sequence,
        title_id: telemetry.title_id,
        gamertag: telemetry.gamertag,
        achievements_unlocked: telemetry.achievements_unlocked,
        achievements_total: telemetry.achievements_total,
        game_gamerscore: telemetry.game_gamerscore,
        total_gamerscore: telemetry.total_gamerscore,
        cpu_celsius: telemetry.cpu_celsius,
        gpu_celsius: telemetry.gpu_celsius,
    }
}

fn unsigned_from_wire(wire: &WireTelemetry) -> UnsignedTelemetry {
    UnsignedTelemetry {
        version: wire.version,
        message_type: wire.message_type.clone(),
        sequence: wire.sequence,
        title_id: wire.title_id.clone(),
        gamertag: wire.gamertag.clone(),
        achievements_unlocked: wire.achievements_unlocked,
        achievements_total: wire.achievements_total,
        game_gamerscore: wire.game_gamerscore,
        total_gamerscore: wire.total_gamerscore,
        cpu_celsius: wire.cpu_celsius,
        gpu_celsius: wire.gpu_celsius,
    }
}

fn telemetry_mac(
    key: &SharedKey,
    challenge: &Challenge,
    sequence: u64,
    canonical_body: &[u8],
) -> [u8; 32] {
    let mut input = Vec::with_capacity(TELEMETRY_DOMAIN.len() + 32 + 8 + canonical_body.len());
    input.extend_from_slice(TELEMETRY_DOMAIN);
    input.extend_from_slice(&challenge.0);
    input.extend_from_slice(&sequence.to_be_bytes());
    input.extend_from_slice(canonical_body);
    hmac_sha256(&key.0, &input)
}

fn hmac_sha256(key_bytes: &[u8], input: &[u8]) -> [u8; 32] {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key_bytes);
    let signature = hmac::sign(&key, input);
    signature
        .as_ref()
        .try_into()
        .expect("SHA-256 MAC has 32 bytes")
}

fn parse_canonical<T>(payload: &[u8]) -> Result<T, ProtocolError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let message: T = serde_json::from_slice(payload).map_err(|_| ProtocolError::InvalidJson)?;
    let canonical = serde_json::to_vec(&message).map_err(|_| ProtocolError::InvalidJson)?;
    if canonical != payload {
        return Err(ProtocolError::NonCanonicalJson);
    }
    Ok(message)
}

fn validate_fields(telemetry: &CompanionTelemetry) -> Result<(), ProtocolError> {
    if let Some(title_id) = &telemetry.title_id
        && (title_id.len() != 8 || !title_id.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(ProtocolError::InvalidTelemetry("title_id"));
    }
    if let Some(gamertag) = &telemetry.gamertag
        && (gamertag.is_empty()
            || gamertag.chars().count() > 32
            || gamertag.chars().any(char::is_control))
    {
        return Err(ProtocolError::InvalidTelemetry("gamertag"));
    }
    if let (Some(unlocked), Some(total)) = (
        telemetry.achievements_unlocked,
        telemetry.achievements_total,
    ) && unlocked > total
    {
        return Err(ProtocolError::InvalidTelemetry("achievements"));
    }
    for (field, value) in [
        ("cpu_celsius", telemetry.cpu_celsius),
        ("gpu_celsius", telemetry.gpu_celsius),
    ] {
        if value.is_some_and(|temperature| {
            !temperature.is_finite() || !(-50.0..=200.0).contains(&temperature)
        }) {
            return Err(ProtocolError::InvalidTelemetry(field));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        text.push(DIGITS[usize::from(byte >> 4)] as char);
        text.push(DIGITS[usize::from(byte & 0xf)] as char);
    }
    text
}

fn parse_hex_32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = (pair[0] as char).to_digit(16)? as u8;
        let low = (pair[1] as char).to_digit(16)? as u8;
        bytes[index] = (high << 4) | low;
    }
    Some(bytes)
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompanionEvent {
    Connected,
    Snapshot {
        sequence: u64,
        snapshot: TelemetrySnapshot,
    },
    Disconnected(&'static str),
    Rejected(&'static str),
}

/// A single-session listener. It accepts only authenticated telemetry and has no command path.
pub struct CompanionServer {
    address: SocketAddr,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    events: std::sync::mpsc::Receiver<CompanionEvent>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CompanionServer {
    pub fn bind(
        address: SocketAddr,
        key: SharedKey,
        heartbeat_timeout: std::time::Duration,
    ) -> Result<Self, ProtocolError> {
        if !is_private_or_loopback(address.ip()) || heartbeat_timeout.is_zero() {
            return Err(ProtocolError::InvalidTelemetry("listener configuration"));
        }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_thread = std::sync::Arc::clone(&stop);
        let (event_tx, events) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("xerust360-companion".to_owned())
            .spawn(move || {
                while !stop_thread.load(std::sync::atomic::Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _peer)) => {
                            let _ = handle_connection(
                                stream,
                                &key,
                                heartbeat_timeout,
                                &stop_thread,
                                &event_tx,
                            );
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(25));
                        }
                        Err(_) => {
                            let _ = event_tx.send(CompanionEvent::Disconnected("Listener failed"));
                            break;
                        }
                    }
                }
            })
            .map_err(ProtocolError::from)?;
        Ok(Self {
            address,
            stop,
            events,
            thread: Some(thread),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    pub fn try_event(&self) -> Result<CompanionEvent, TryRecvError> {
        self.events.try_recv()
    }

    pub fn stop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for CompanionServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A mock peer for local tests. It sends a finite sequence and closes its title session.
pub fn send_mock_records(
    address: SocketAddr,
    key: &SharedKey,
    records: &[CompanionTelemetry],
) -> Result<(), ProtocolError> {
    let mut stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    let challenge_frame = read_frame(&mut stream)?.ok_or(ProtocolError::InvalidFrameLength)?;
    let challenge = Challenge::from_message(decode_frame(&challenge_frame)?)?;
    write_wire_frame(&mut stream, &encode_authentication(key, &challenge))?;
    for (index, record) in records.iter().enumerate() {
        let sequence = u64::try_from(index + 1).map_err(|_| ProtocolError::InvalidSequence)?;
        let frame = encode_telemetry(key, &challenge, sequence, record)?;
        stream.write_all(&frame)?;
    }
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    key: &SharedKey,
    heartbeat_timeout: std::time::Duration,
    stop: &std::sync::atomic::AtomicBool,
    events: &std::sync::mpsc::Sender<CompanionEvent>,
) -> Result<(), ProtocolError> {
    stream.set_read_timeout(Some(std::time::Duration::from_millis(250)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    let challenge = Challenge::generate()?;
    stream.write_all(&challenge.message_frame()?)?;
    let auth_deadline = Instant::now() + std::time::Duration::from_secs(5);
    let auth_payload = loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) || Instant::now() >= auth_deadline {
            let _ = events.send(CompanionEvent::Rejected("Authentication timed out"));
            return Ok(());
        }
        match read_frame(&mut stream) {
            Ok(Some(frame)) => break decode_frame(&frame)?.to_vec(),
            Ok(None) => continue,
            Err(_) => {
                let _ = events.send(CompanionEvent::Rejected("Invalid authentication frame"));
                return Ok(());
            }
        }
    };
    if verify_authentication(key, &challenge, &auth_payload).is_err() {
        let _ = events.send(CompanionEvent::Rejected("Authentication failed"));
        return Ok(());
    }
    let _ = events.send(CompanionEvent::Connected);
    let mut session = AuthenticatedSession::default();
    let mut last_valid = Instant::now();
    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = events.send(CompanionEvent::Disconnected("Listener stopped"));
            return Ok(());
        }
        match read_frame(&mut stream) {
            Ok(Some(frame)) => match session.verify_next(key, &challenge, &frame) {
                Ok((sequence, snapshot)) => {
                    last_valid = Instant::now();
                    if events
                        .send(CompanionEvent::Snapshot { sequence, snapshot })
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                Err(_) => {
                    let _ = events.send(CompanionEvent::Rejected("Invalid telemetry frame"));
                    let _ = events.send(CompanionEvent::Disconnected("Companion session ended"));
                    return Ok(());
                }
            },
            Ok(None) => {
                if Instant::now().duration_since(last_valid) >= heartbeat_timeout {
                    let _ =
                        events.send(CompanionEvent::Disconnected("Telemetry heartbeat expired"));
                    return Ok(());
                }
            }
            Err(_) => {
                let _ = events.send(CompanionEvent::Disconnected("Companion session ended"));
                return Ok(());
            }
        }
    }
}

fn read_frame(stream: &mut TcpStream) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut header = [0; 4];
    match stream.read_exact(&mut header[..1]) {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) =>
        {
            return Ok(None);
        }
        // EOF means the peer's title process/session ended. Only a socket read
        // timeout is an idle heartbeat interval; treating EOF as idle delays
        // disconnect handling until the heartbeat deadline.
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(ProtocolError::from(error));
        }
        Err(error) => return Err(ProtocolError::from(error)),
    }
    stream.read_exact(&mut header[1..])?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 {
        return Err(ProtocolError::InvalidFrameLength);
    }
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::OversizedFrame);
    }
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload)?;
    let mut frame = Vec::with_capacity(4 + length);
    frame.extend_from_slice(&header);
    frame.extend_from_slice(&payload);
    Ok(Some(frame))
}

fn write_wire_frame(stream: &mut TcpStream, payload: &[u8]) -> Result<(), ProtocolError> {
    stream.write_all(&encode_frame(payload)?)?;
    Ok(())
}

fn is_private_or_loopback(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::{Ipv4Addr, SocketAddrV4},
        thread,
        time::Duration,
    };

    fn key() -> SharedKey {
        SharedKey::new([0x5a; 32])
    }

    fn challenge() -> Challenge {
        Challenge::from_bytes([0xa5; 32])
    }

    #[test]
    fn hmac_matches_rfc_4231_sha256_vector() {
        let key = [0x0b; 20];
        let result = hmac_sha256(&key, b"Hi There");
        assert_eq!(
            hex(&result),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn frames_are_bounded_and_require_exact_length() {
        let framed = encode_frame(b"abc").unwrap();
        assert_eq!(&framed[..4], &3_u32.to_be_bytes());
        assert_eq!(decode_frame(&framed).unwrap(), b"abc");
        assert_eq!(
            decode_frame(&framed[..6]),
            Err(ProtocolError::InvalidFrameLength)
        );
        assert_eq!(
            encode_frame(&vec![0; MAX_FRAME_BYTES + 1]),
            Err(ProtocolError::OversizedFrame)
        );
    }

    #[test]
    fn authentication_proof_is_challenge_and_key_bound() {
        let key = key();
        let proof = encode_authentication(&key, &challenge());
        verify_authentication(&key, &challenge(), &proof).unwrap();
        assert_eq!(
            verify_authentication(&key, &Challenge::from_bytes([0; 32]), &proof),
            Err(ProtocolError::InvalidMac)
        );
        assert_eq!(
            verify_authentication(&SharedKey::new([0; 32]), &challenge(), &proof),
            Err(ProtocolError::InvalidMac)
        );
    }

    #[test]
    fn canonical_telemetry_round_trips_and_missing_fields_stay_missing() {
        let key = key();
        let challenge = challenge();
        let mut session = AuthenticatedSession::default();
        let telemetry = CompanionTelemetry {
            title_id: Some("4D5307E6".to_owned()),
            ..CompanionTelemetry::default()
        };
        let frame = encode_telemetry(&key, &challenge, 1, &telemetry).unwrap();
        let (sequence, snapshot) = session.verify_next(&key, &challenge, &frame).unwrap();
        assert_eq!(sequence, 1);
        assert_eq!(snapshot.game.title_id.as_deref(), Some("4D5307E6"));
        assert_eq!(snapshot.gamertag, None);
        assert_eq!(snapshot.achievements.total, None);
        assert_eq!(snapshot.temperatures.cpu_celsius, None);
    }

    #[test]
    fn telemetry_serialization_matches_v1_golden_vector() {
        let encoded = encode_telemetry(
            &key(),
            &challenge(),
            1,
            &CompanionTelemetry {
                title_id: Some("4D5307E6".to_owned()),
                ..CompanionTelemetry::default()
            },
        )
        .unwrap();
        let payload = decode_frame(&encoded).unwrap();
        assert_eq!(
            std::str::from_utf8(payload).unwrap(),
            r#"{"version":1,"message_type":"telemetry","sequence":1,"title_id":"4D5307E6","gamertag":null,"achievements_unlocked":null,"achievements_total":null,"game_gamerscore":null,"total_gamerscore":null,"cpu_celsius":null,"gpu_celsius":null,"mac":"5e2b60cf2e149311b1c0c9ebec8b1ec658c281a8337a7d0f0ebaa93e2f060644"}"#
        );
    }

    #[test]
    fn replay_and_tampering_are_rejected_without_advancing_sequence() {
        let key = key();
        let challenge = challenge();
        let telemetry = CompanionTelemetry {
            title_id: Some("4D5307E6".to_owned()),
            ..Default::default()
        };
        let frame = encode_telemetry(&key, &challenge, 5, &telemetry).unwrap();
        let mut session = AuthenticatedSession::default();
        session.verify_next(&key, &challenge, &frame).unwrap();
        assert_eq!(
            session.verify_next(&key, &challenge, &frame),
            Err(ProtocolError::InvalidSequence)
        );

        let mut tampered = encode_telemetry(&key, &challenge, 6, &telemetry).unwrap();
        let title_byte = tampered.iter().position(|byte| *byte == b'4').unwrap();
        tampered[title_byte] = b'5';
        assert_eq!(
            session.verify_next(&key, &challenge, &tampered),
            Err(ProtocolError::InvalidMac)
        );
        let valid = encode_telemetry(&key, &challenge, 6, &telemetry).unwrap();
        session.verify_next(&key, &challenge, &valid).unwrap();
    }

    #[test]
    fn invalid_versions_unknown_fields_and_bad_values_are_rejected() {
        let key = key();
        let challenge = challenge();
        let mut session = AuthenticatedSession::default();
        let mut frame =
            encode_telemetry(&key, &challenge, 1, &CompanionTelemetry::default()).unwrap();
        let payload = std::str::from_utf8(&frame[4..])
            .unwrap()
            .replace("\"version\":1", "\"version\":2");
        frame = encode_frame(payload.as_bytes()).unwrap();
        assert_eq!(
            session.verify_next(&key, &challenge, &frame),
            Err(ProtocolError::WrongVersion)
        );
        let auth = encode_authentication(&key, &challenge);
        let unknown = std::str::from_utf8(&auth)
            .unwrap()
            .replace("\"mac\":", "\"extra\":0,\"mac\":");
        assert_eq!(
            verify_authentication(&key, &challenge, unknown.as_bytes()),
            Err(ProtocolError::InvalidJson)
        );
        let spaced = std::str::from_utf8(&auth).unwrap().replace(',', ", ");
        assert_eq!(
            verify_authentication(&key, &challenge, spaced.as_bytes()),
            Err(ProtocolError::NonCanonicalJson)
        );
        let duplicate = std::str::from_utf8(&auth)
            .unwrap()
            .replace("\"version\":1,", "\"version\":1,\"version\":1,");
        assert_eq!(
            verify_authentication(&key, &challenge, duplicate.as_bytes()),
            Err(ProtocolError::InvalidJson)
        );
        assert!(
            CompanionTelemetry {
                title_id: Some("not-a-title".to_owned()),
                ..Default::default()
            }
            .into_snapshot()
            .is_err()
        );
        assert!(
            CompanionTelemetry {
                achievements_unlocked: Some(2),
                achievements_total: Some(1),
                ..Default::default()
            }
            .into_snapshot()
            .is_err()
        );
    }

    #[test]
    fn debug_output_redacts_shared_key_and_diagnostics_are_generic() {
        let key = key();
        assert!(!format!("{key:?}").contains("5a"));
        assert!(!ProtocolError::InvalidMac.to_string().contains("5a"));
    }

    fn next_event(server: &CompanionServer) -> CompanionEvent {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match server.try_event() {
                Ok(event) => return event,
                Err(TryRecvError::Disconnected) => panic!("companion event channel closed"),
                Err(TryRecvError::Empty) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(TryRecvError::Empty) => panic!("timed out waiting for companion event"),
            }
        }
    }

    fn loopback_server(key: SharedKey, timeout: Duration) -> Option<CompanionServer> {
        match CompanionServer::bind(
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
            key,
            timeout,
        ) {
            Ok(server) => Some(server),
            Err(ProtocolError::Io(io::ErrorKind::PermissionDenied)) => {
                const REQUIRE_LOOPBACK: &str = "XERUST_REQUIRE_LOOPBACK";
                if std::env::var_os(REQUIRE_LOOPBACK).is_some() {
                    panic!("required localhost TCP socket is unavailable (PermissionDenied)");
                }
                println!(
                    "SKIPPED: localhost TCP socket unavailable (PermissionDenied); set {REQUIRE_LOOPBACK}=1 to make this a failure"
                );
                None
            }
            Err(error) => panic!("could not bind test listener: {error:?}"),
        }
    }

    #[test]
    fn loopback_mock_peer_drives_authenticated_session_and_disconnect() {
        let key = key();
        let Some(server) = loopback_server(key.clone(), Duration::from_secs(1)) else {
            return;
        };
        send_mock_records(
            server.local_addr(),
            &key,
            &[CompanionTelemetry {
                title_id: Some("4D5307E6".to_owned()),
                gamertag: Some("MockPlayer".to_owned()),
                ..Default::default()
            }],
        )
        .unwrap();
        assert_eq!(next_event(&server), CompanionEvent::Connected);
        match next_event(&server) {
            CompanionEvent::Snapshot { sequence, snapshot } => {
                assert_eq!(sequence, 1);
                assert_eq!(snapshot.game.title_id.as_deref(), Some("4D5307E6"));
                assert_eq!(snapshot.gamertag.as_deref(), Some("MockPlayer"));
            }
            event => panic!("unexpected event: {event:?}"),
        }
        assert_eq!(
            next_event(&server),
            CompanionEvent::Disconnected("Companion session ended")
        );
    }

    #[test]
    fn authenticated_peer_with_invalid_message_is_rejected_and_disconnected() {
        let key = key();
        let Some(server) = loopback_server(key.clone(), Duration::from_secs(2)) else {
            return;
        };
        let mut stream = TcpStream::connect(server.local_addr()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let challenge_frame = read_frame(&mut stream).unwrap().unwrap();
        let challenge = Challenge::from_message(decode_frame(&challenge_frame).unwrap()).unwrap();
        write_wire_frame(&mut stream, &encode_authentication(&key, &challenge)).unwrap();
        assert_eq!(next_event(&server), CompanionEvent::Connected);

        stream.write_all(&encode_frame(b"{}").unwrap()).unwrap();
        assert_eq!(
            next_event(&server),
            CompanionEvent::Rejected("Invalid telemetry frame")
        );
        assert_eq!(
            next_event(&server),
            CompanionEvent::Disconnected("Companion session ended")
        );
    }

    #[test]
    fn idle_companion_expires_and_invalid_key_is_rejected() {
        let key = key();
        let Some(server) = loopback_server(key.clone(), Duration::from_millis(350)) else {
            return;
        };
        let mut stream = TcpStream::connect(server.local_addr()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let challenge_frame = read_frame(&mut stream).unwrap().unwrap();
        let challenge = Challenge::from_message(decode_frame(&challenge_frame).unwrap()).unwrap();
        write_wire_frame(&mut stream, &encode_authentication(&key, &challenge)).unwrap();
        assert_eq!(next_event(&server), CompanionEvent::Connected);
        assert_eq!(
            next_event(&server),
            CompanionEvent::Disconnected("Telemetry heartbeat expired")
        );

        let Some(bad_server) = loopback_server(key.clone(), Duration::from_secs(1)) else {
            return;
        };
        send_mock_records(bad_server.local_addr(), &SharedKey::new([0; 32]), &[]).unwrap();
        assert_eq!(
            next_event(&bad_server),
            CompanionEvent::Rejected("Authentication failed")
        );
    }

    #[test]
    fn listener_refuses_non_private_bind_addresses() {
        let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 4), 0));
        assert!(CompanionServer::bind(address, key(), Duration::from_secs(1)).is_err());
    }

    #[test]
    fn shared_keys_require_exact_256_bit_hex_and_redact_debug() {
        let key = SharedKey::from_hex(&"ab".repeat(32)).unwrap();
        assert_eq!(key.to_hex(), "ab".repeat(32));
        assert!(SharedKey::from_hex("ab").is_err());
        assert!(SharedKey::from_hex(&"zz".repeat(32)).is_err());
        assert_eq!(format!("{key:?}"), "SharedKey([REDACTED])");
    }
}
