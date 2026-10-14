use std::{
    fmt,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    process::Command,
    sync::mpsc,
    time::Duration,
};

use serde_json::Value;

use crate::domain::{AchievementProgress, GameIdentity, TelemetrySnapshot, Temperatures};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const DISCOVERY_TIMEOUT: Duration = Duration::from_millis(160);
const DISCOVERY_MAX_HOSTS: usize = 512;
const DISCOVERY_WORKERS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetrySchedule {
    pub initial: Duration,
    pub maximum: Duration,
}

impl Default for RetrySchedule {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            maximum: Duration::from_secs(30),
        }
    }
}

impl RetrySchedule {
    pub fn delay(self, failed_attempt: u32) -> Duration {
        let multiplier = 1u32.checked_shl(failed_attempt.min(31)).unwrap_or(u32::MAX);
        self.initial.saturating_mul(multiplier).min(self.maximum)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NovaEndpoint {
    pub host: Ipv4Addr,
    pub port: u16,
}

/// Discover HTTP responders on local private IPv4 networks only.
/// Host enumeration is capped per interface; no public address ranges are scanned.
pub fn discover_nova_endpoints(port: u16) -> Vec<NovaEndpoint> {
    let candidates = local_private_hosts();
    if candidates.is_empty() {
        return Vec::new();
    }
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        for worker in 0..DISCOVERY_WORKERS.min(candidates.len()) {
            let sender = sender.clone();
            let candidates = &candidates;
            scope.spawn(move || {
                for host in candidates.iter().skip(worker).step_by(DISCOVERY_WORKERS) {
                    if probe_http_service(*host, port) {
                        let _ = sender.send(NovaEndpoint { host: *host, port });
                    }
                }
            });
        }
    });
    drop(sender);
    let mut endpoints: Vec<_> = receiver.into_iter().collect();
    endpoints.sort_by_key(|endpoint| endpoint.host);
    endpoints
}

fn local_private_hosts() -> Vec<Ipv4Addr> {
    let output = interface_address_output();
    let mut hosts = Vec::new();
    for item in output {
        let Some((address, prefix)) = parse_interface_cidr(&item) else {
            continue;
        };
        if !is_private_ipv4(address) || !(24..=30).contains(&prefix) {
            continue;
        }
        hosts.extend(hosts_in_subnet(address, prefix));
    }
    hosts.sort_unstable();
    hosts.dedup();
    hosts
}

#[cfg(target_os = "linux")]
fn interface_address_output() -> Vec<String> {
    Command::new("ip")
        .args(["-o", "-f", "inet", "addr", "show", "scope", "global"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn interface_address_output() -> Vec<String> {
    Command::new("ifconfig")
        .arg("-a")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn interface_address_output() -> Vec<String> {
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "Get-NetIPAddress -AddressFamily IPv4 | ForEach-Object { \"$($_.IPAddress)/$($_.PrefixLength)\" }",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn interface_address_output() -> Vec<String> {
    Vec::new()
}

fn parse_interface_cidr(line: &str) -> Option<(Ipv4Addr, u8)> {
    if let Some(cidr) = line.split_whitespace().find(|part| part.contains('/')) {
        return parse_cidr(cidr);
    }
    // macOS ifconfig emits `inet A.B.C.D netmask 0xffffff00`.
    let words: Vec<_> = line.split_whitespace().collect();
    let address = words.iter().position(|word| *word == "inet")?;
    let ip = words.get(address + 1)?.parse().ok()?;
    let mask_index = words.iter().position(|word| *word == "netmask")?;
    let mask = u32::from_str_radix(words.get(mask_index + 1)?.trim_start_matches("0x"), 16).ok()?;
    let prefix = mask.leading_ones() as u8;
    Some((ip, prefix))
}

fn parse_cidr(value: &str) -> Option<(Ipv4Addr, u8)> {
    let (address, prefix) = value.split_once('/')?;
    Some((address.parse().ok()?, prefix.parse().ok()?))
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

fn hosts_in_subnet(address: Ipv4Addr, prefix: u8) -> Vec<Ipv4Addr> {
    if prefix > 30 {
        return vec![address];
    }
    let address = u32::from(address);
    let mask = u32::MAX << (32 - prefix);
    let network = address & mask;
    let end = network | !mask;
    let start = network.saturating_add(1);
    let stop = end.min(start.saturating_add(DISCOVERY_MAX_HOSTS as u32 - 1));
    (start..=stop)
        .filter(|host| *host != address && *host < end)
        .map(Ipv4Addr::from)
        .collect()
}

fn probe_http_service(host: Ipv4Addr, port: u16) -> bool {
    let address = SocketAddr::new(IpAddr::V4(host), port);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, DISCOVERY_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(DISCOVERY_TIMEOUT));
    let _ = stream.set_write_timeout(Some(DISCOVERY_TIMEOUT));
    if stream
        .write_all(
            format!("GET /plugin HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .is_err()
    {
        return false;
    }
    let mut response = [0u8; 16];
    stream
        .read(&mut response)
        .is_ok_and(|length| response[..length].starts_with(b"HTTP/1."))
}

#[derive(Clone)]
pub struct NovaCredentials {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for NovaCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NovaCredentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct NovaConfig {
    pub host: IpAddr,
    pub port: u16,
    pub credentials: Option<NovaCredentials>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NovaError {
    InvalidHost,
    Request(String),
    Unauthorized,
    HttpStatus(u16),
    InvalidResponse(&'static str),
}

impl fmt::Display for NovaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHost => write!(f, "NOVA host must be an IP address"),
            Self::Request(message) => write!(f, "NOVA request failed: {message}"),
            Self::Unauthorized => write!(f, "NOVA authentication failed; check its credentials"),
            Self::HttpStatus(status) => write!(f, "NOVA returned HTTP status {status}"),
            Self::InvalidResponse(endpoint) => {
                write!(f, "NOVA returned invalid data from {endpoint}")
            }
        }
    }
}

impl std::error::Error for NovaError {}

#[derive(Clone, Debug)]
pub struct PollDiagnostics {
    pub title: Result<(), String>,
    pub profile: Result<(), String>,
    pub achievements: Result<(), String>,
    pub temperatures: Result<(), String>,
}

impl Default for PollDiagnostics {
    fn default() -> Self {
        Self {
            title: Ok(()),
            profile: Ok(()),
            achievements: Ok(()),
            temperatures: Ok(()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PollResult {
    pub snapshot: TelemetrySnapshot,
    pub diagnostics: PollDiagnostics,
}

pub struct NovaClient {
    config: NovaConfig,
    token: Option<String>,
}

impl NovaClient {
    pub fn new(config: NovaConfig) -> Self {
        Self {
            config,
            token: None,
        }
    }

    pub fn poll(&mut self) -> Result<PollResult, NovaError> {
        let title = self.get_json("/title");
        let profile = self.get_json("/profile");
        let achievements = self.get_json("/achievement");
        let player_achievements = self.get_json("/achievement/player");
        let temperatures = self.get_json("/temperature");

        let mut result = PollResult::default();
        let mut selected_profile_index = None;
        match title {
            Ok(value) => {
                result.snapshot.game = parse_title(&value);
                result.diagnostics.title = Ok(());
            }
            Err(error) => result.diagnostics.title = Err(error.to_string()),
        }
        match profile {
            Ok(value) => {
                let (gamertag, gamerscore, profile_index) = parse_profile(&value);
                result.snapshot.gamertag = gamertag;
                result.snapshot.gamerscore.total = gamerscore;
                selected_profile_index = profile_index;
                result.diagnostics.profile = Ok(());
            }
            Err(error) => result.diagnostics.profile = Err(error.to_string()),
        }
        match (achievements, player_achievements) {
            (Ok(catalog), Ok(player)) => {
                let (progress, score) =
                    parse_achievements(&catalog, &player, selected_profile_index);
                result.snapshot.achievements = progress;
                result.snapshot.gamerscore.game = score;
                result.diagnostics.achievements = Ok(());
            }
            (catalog, player) => {
                result.diagnostics.achievements = Err(match (catalog, player) {
                    (Err(error), _) | (_, Err(error)) => error.to_string(),
                    _ => "achievement data unavailable".to_owned(),
                });
            }
        }
        match temperatures {
            Ok(value) => {
                result.snapshot.temperatures = parse_temperatures(&value);
                result.diagnostics.temperatures = Ok(());
            }
            Err(error) => result.diagnostics.temperatures = Err(error.to_string()),
        }

        if result.diagnostics.title.is_err()
            && result.diagnostics.profile.is_err()
            && result.diagnostics.achievements.is_err()
            && result.diagnostics.temperatures.is_err()
        {
            return Err(NovaError::Request("no NOVA endpoints responded".to_owned()));
        }
        Ok(result)
    }

    fn authenticate(&mut self) -> Result<(), NovaError> {
        let Some(credentials) = &self.config.credentials else {
            return Ok(());
        };
        let boundary = format!("----XeRust360{}", std::process::id());
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"username\"\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"password\"\r\n\r\n{}\r\n--{boundary}--\r\n",
            credentials.username, credentials.password
        );
        let response = self.request(
            "POST",
            "/authenticate",
            Some(&format!("multipart/form-data; boundary={boundary}")),
            Some(&body),
            None,
        )?;
        let value = response_json(response, "/authenticate")?;
        self.token = value
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if self.token.is_none() {
            return Err(NovaError::InvalidResponse("/authenticate"));
        }
        Ok(())
    }

    fn get_json(&mut self, endpoint: &'static str) -> Result<Value, NovaError> {
        if self.config.credentials.is_some() && self.token.is_none() {
            self.authenticate()?;
        }
        let mut retried_auth = false;
        loop {
            let response = self.request("GET", endpoint, None, None, self.token.as_deref())?;
            if response.status == 401 {
                if self.config.credentials.is_some() && !retried_auth {
                    self.token = None;
                    self.authenticate()?;
                    retried_auth = true;
                    continue;
                }
                return Err(NovaError::Unauthorized);
            }
            return response_json(response, endpoint);
        }
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: Option<&str>,
        token: Option<&str>,
    ) -> Result<HttpResponse, NovaError> {
        let address = SocketAddr::new(self.config.host, self.config.port);
        let mut stream = TcpStream::connect_timeout(&address, REQUEST_TIMEOUT).map_err(|_| {
            NovaError::Request("connection timed out or host is unreachable".to_owned())
        })?;
        stream
            .set_read_timeout(Some(REQUEST_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(REQUEST_TIMEOUT)))
            .map_err(|_| NovaError::Request("could not set request timeout".to_owned()))?;

        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n",
            address
        );
        if let Some(content_type) = content_type {
            request.push_str(&format!("Content-Type: {content_type}\r\n"));
        }
        if let Some(token) = token {
            request.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        if let Some(body) = body {
            request.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .and_then(|()| match body {
                Some(body) => stream.write_all(body.as_bytes()),
                None => Ok(()),
            })
            .map_err(|_| NovaError::Request("failed to send request".to_owned()))?;
        let mut bytes = Vec::new();
        stream
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|_| NovaError::Request("failed to read response".to_owned()))?;
        if bytes.len() > 1_048_576 {
            return Err(NovaError::Request("response was too large".to_owned()));
        }
        parse_http_response(&bytes)
    }
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

fn parse_http_response(bytes: &[u8]) -> Result<HttpResponse, NovaError> {
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(NovaError::InvalidResponse("HTTP"))?;
    let header =
        std::str::from_utf8(&bytes[..split]).map_err(|_| NovaError::InvalidResponse("HTTP"))?;
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(NovaError::InvalidResponse("HTTP"))?;
    let body = &bytes[split + 4..];
    let is_chunked = header.lines().any(|line| {
        line.split_once(':').is_some_and(|(key, value)| {
            key.eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
        })
    });
    let body = if is_chunked {
        decode_chunked(body)?
    } else if let Some(length) = header.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    }) {
        body.get(..length)
            .ok_or(NovaError::InvalidResponse("HTTP"))?
            .to_vec()
    } else {
        body.to_vec()
    };
    Ok(HttpResponse { status, body })
}

fn decode_chunked(input: &[u8]) -> Result<Vec<u8>, NovaError> {
    let mut output = Vec::new();
    let mut cursor = 0;
    loop {
        let line_end = input[cursor..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|offset| cursor + offset)
            .ok_or(NovaError::InvalidResponse("HTTP"))?;
        let length_text = std::str::from_utf8(&input[cursor..line_end])
            .map_err(|_| NovaError::InvalidResponse("HTTP"))?
            .split(';')
            .next()
            .unwrap_or("");
        let length = usize::from_str_radix(length_text.trim(), 16)
            .map_err(|_| NovaError::InvalidResponse("HTTP"))?;
        cursor = line_end + 2;
        if length == 0 {
            return Ok(output);
        }
        let end = cursor
            .checked_add(length)
            .ok_or(NovaError::InvalidResponse("HTTP"))?;
        output.extend_from_slice(
            input
                .get(cursor..end)
                .ok_or(NovaError::InvalidResponse("HTTP"))?,
        );
        if input.get(end..end + 2) != Some(b"\r\n") {
            return Err(NovaError::InvalidResponse("HTTP"));
        }
        cursor = end + 2;
    }
}

fn response_json(response: HttpResponse, endpoint: &'static str) -> Result<Value, NovaError> {
    if response.status == 204 {
        return Ok(Value::Array(Vec::new()));
    }
    if !(200..300).contains(&response.status) {
        return Err(NovaError::HttpStatus(response.status));
    }
    serde_json::from_slice(&response.body).map_err(|_| NovaError::InvalidResponse(endpoint))
}

fn parse_title(value: &Value) -> GameIdentity {
    let title_id = field(value, &["titleid", "title_id"])
        .and_then(value_to_string)
        .filter(|s| s != "0" && s != "0x00000000");
    let name = field(value, &["title_name", "gametitle", "name"])
        .and_then(value_to_string)
        .filter(|s| !s.trim().is_empty());
    GameIdentity {
        title_id,
        name,
        ..Default::default()
    }
}

fn parse_profile(value: &Value) -> (Option<String>, Option<u32>, Option<usize>) {
    let entries = value
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![value.clone()]);
    let signed_in = entries.iter().find(|entry| {
        field(entry, &["signedin", "signed_in"])
            .and_then(value_to_u32)
            .is_some_and(|value| value == 1)
    });
    let Some(profile) = signed_in else {
        return (None, None, None);
    };
    (
        field(profile, &["gamertag", "gamertagname"])
            .and_then(value_to_string)
            .filter(|tag| !tag.trim().is_empty()),
        field(profile, &["gamerscore", "total_gamerscore"]).and_then(value_to_u32),
        field(profile, &["index", "player_index"])
            .and_then(value_to_u32)
            .and_then(|index| usize::try_from(index).ok())
            .filter(|index| *index < 4),
    )
}

fn parse_achievements(
    catalog: &Value,
    player: &Value,
    player_index: Option<usize>,
) -> (AchievementProgress, Option<u32>) {
    let all = catalog.as_array();
    let earned = player.as_array();
    let total = all.and_then(|items| u32::try_from(items.len()).ok());
    let Some(all) = all else {
        return (AchievementProgress::default(), None);
    };
    if all.is_empty() {
        return (AchievementProgress::default(), None);
    }
    let Some(earned) = earned else {
        return (
            AchievementProgress {
                unlocked: None,
                total,
            },
            None,
        );
    };
    let Some(player_index) = player_index else {
        return (
            AchievementProgress {
                unlocked: None,
                total,
            },
            None,
        );
    };

    let mut unlocked = 0u32;
    let mut game_score = 0u32;
    let mut score_known = true;
    let mut unlock_known = true;
    for achievement in all {
        let id = field(achievement, &["id"]).and_then(value_to_u32);
        let progress = earned
            .iter()
            .find(|progress| id.is_some() && field(progress, &["id"]).and_then(value_to_u32) == id);
        let unlocked_flag = progress
            .and_then(|progress| field(progress, &["player"]))
            .and_then(Value::as_array)
            .and_then(|players| players.get(player_index))
            .and_then(value_to_u32)
            .filter(|value| *value <= 1);
        let Some(is_unlocked) = unlocked_flag else {
            unlock_known = false;
            continue;
        };
        if is_unlocked == 0 {
            continue;
        }
        unlocked += 1;
        let score = field(achievement, &["cred"]).and_then(value_to_u32);
        if let Some(score) = score {
            game_score = game_score.saturating_add(score);
        } else {
            score_known = false;
        }
    }
    (
        AchievementProgress {
            unlocked: unlock_known.then_some(unlocked),
            total,
        },
        (unlock_known && score_known).then_some(game_score),
    )
}

fn parse_temperatures(value: &Value) -> Temperatures {
    let celsius = field(value, &["celsius"]).and_then(Value::as_bool);
    let normalize = |temperature: Option<f32>| match celsius {
        Some(true) => temperature,
        Some(false) => temperature.map(|fahrenheit| (fahrenheit - 32.0) * (5.0 / 9.0)),
        None => None,
    };
    Temperatures {
        cpu_celsius: normalize(recursive_number(
            value,
            &["cpu", "cpu_temp", "cputemp", "cpu_temperature"],
        )),
        gpu_celsius: normalize(recursive_number(
            value,
            &["gpu", "gpu_temp", "gputemp", "gpu_temperature"],
        )),
    }
}

fn field<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;
    names.iter().find_map(|name| {
        object
            .iter()
            .find_map(|(key, value)| (normalize_key(key) == normalize_key(name)).then_some(value))
    })
}

fn recursive_number(value: &Value, names: &[&str]) -> Option<f32> {
    if let Some(found) = field(value, names).and_then(value_to_f32) {
        return Some(found);
    }
    match value {
        Value::Array(values) => values.iter().find_map(|item| recursive_number(item, names)),
        Value::Object(values) => values
            .values()
            .find_map(|item| recursive_number(item, names)),
        _ => None,
    }
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_to_u32(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|value| u32::try_from(value).ok())
}

fn value_to_f32(value: &Value) -> Option<f32> {
    let number = value.as_f64()? as f32;
    number.is_finite().then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nova_title_and_profile_schema_fields() {
        let title = serde_json::json!({"titleid":"0x41560817", "mediaid":"0x0"});
        assert_eq!(parse_title(&title).title_id.as_deref(), Some("0x41560817"));
        let profiles = serde_json::json!([
            {"gamertag":"Offline", "gamerscore":900, "signedin":0, "index":0},
            {"gamertag":"PlayerOne", "gamerscore":12000, "signedin":1, "index":1}
        ]);
        assert_eq!(
            parse_profile(&profiles),
            (Some("PlayerOne".into()), Some(12000), Some(1))
        );
    }

    #[test]
    fn achievement_progress_uses_catalog_ids_and_signed_in_player_slot() {
        let catalog = serde_json::json!([
            {"id":1,"cred":10},
            {"id":2,"cred":20},
            {"id":3,"cred":30}
        ]);
        let player = serde_json::json!([
            {"id":3,"player":[0,1,0,0]},
            {"id":1,"player":[0,1,0,0]},
            {"id":2,"player":[1,0,0,0]}
        ]);
        assert_eq!(
            parse_achievements(&catalog, &player, Some(1)),
            (
                AchievementProgress {
                    unlocked: Some(2),
                    total: Some(3)
                },
                Some(40)
            )
        );
        let unknown =
            serde_json::json!([{"id":1,"player":[]}, {"id":2,"player":[]}, {"id":3,"player":[]}]);
        assert_eq!(
            parse_achievements(&catalog, &unknown, Some(1)).0,
            AchievementProgress {
                unlocked: None,
                total: Some(3)
            }
        );
        assert_eq!(
            parse_achievements(&serde_json::json!([]), &serde_json::json!([]), Some(0)).0,
            AchievementProgress::default()
        );
    }

    #[test]
    fn converts_temperatures_from_nova_declared_units() {
        let value = serde_json::json!({"cpu":54.4,"gpu":59.0,"celsius":true});
        let temps = parse_temperatures(&value);
        assert_eq!(temps.cpu_celsius, Some(54.4));
        assert_eq!(temps.gpu_celsius, Some(59.0));
        let fahrenheit =
            parse_temperatures(&serde_json::json!({"cpu":122.0,"gpu":140.0,"celsius":false}));
        assert!((fahrenheit.cpu_celsius.unwrap() - 50.0).abs() < 0.001);
        assert!((fahrenheit.gpu_celsius.unwrap() - 60.0).abs() < 0.001);
        assert_eq!(
            parse_temperatures(&serde_json::json!({"cpu":54.0})).cpu_celsius,
            None
        );
    }

    #[test]
    fn diagnostics_never_format_secret_values() {
        let credentials = NovaCredentials {
            username: "xboxhttp".to_owned(),
            password: "secret-value".to_owned(),
        };
        let debug = format!("{credentials:?}");
        assert!(!debug.contains("secret-value"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn decodes_chunked_http_and_no_content_responses() {
        let response = parse_http_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n8\r\n{\"ok\":1}\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(
            serde_json::from_slice::<Value>(&response.body).unwrap()["ok"],
            1
        );

        let empty = response_json(
            parse_http_response(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n").unwrap(),
            "/achievement",
        )
        .unwrap();
        assert_eq!(empty, Value::Array(vec![]));
    }

    #[test]
    fn discovery_cidr_parser_accepts_supported_private_lans_only() {
        assert_eq!(
            parse_cidr("192.168.1.20/24"),
            Some(([192, 168, 1, 20].into(), 24))
        );
        assert_eq!(
            parse_interface_cidr("2: eth0 inet 192.168.1.20/24 brd 192.168.1.255"),
            Some(([192, 168, 1, 20].into(), 24))
        );
        assert!(is_private_ipv4("10.0.0.5".parse().unwrap()));
        assert!(!is_private_ipv4("8.8.8.8".parse().unwrap()));
        assert!(
            hosts_in_subnet("192.168.1.20".parse().unwrap(), 24)
                .contains(&"192.168.1.50".parse().unwrap())
        );
    }

    #[test]
    fn reconnect_delay_grows_exponentially_and_is_bounded() {
        let retry = RetrySchedule::default();
        assert_eq!(retry.delay(0), Duration::from_secs(1));
        assert_eq!(retry.delay(3), Duration::from_secs(8));
        assert_eq!(retry.delay(20), Duration::from_secs(30));
    }
}
