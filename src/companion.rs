//! Versioned, authenticated, read-only LAN companion protocol.
//!
//! This module implements protocol primitives only. It does not load, build,
//! install, or deploy an Xbox executable.

use std::{fmt, io};

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
    Io(String),
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
    fn from(_: io::Error) -> Self {
        Self::Io("redacted".to_owned())
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
