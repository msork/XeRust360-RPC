use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::StateToggles;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub nova_host: String,
    pub nova_port: u16,
    pub nova_username: String,
    pub poll_interval_seconds: u64,
    pub auto_reconnect: bool,
    pub mock_mode: bool,
    pub companion_mode: bool,
    pub companion_bind_address: String,
    pub companion_port: u16,
    pub companion_heartbeat_seconds: u64,
    pub discord_application_id: String,
    pub toggles: StateToggles,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            nova_host: String::new(),
            nova_port: 9999,
            nova_username: String::new(),
            poll_interval_seconds: 10,
            auto_reconnect: true,
            mock_mode: true,
            companion_mode: false,
            companion_bind_address: String::new(),
            companion_port: 17999,
            companion_heartbeat_seconds: 5,
            discord_application_id: String::new(),
            toggles: StateToggles::default(),
        }
    }
}

pub fn settings_path() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join("XeRust360-RPC")
        .join("settings.json")
}

pub fn load() -> AppSettings {
    fs::read(settings_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save(settings: &AppSettings) -> io::Result<()> {
    let path = settings_path();
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("settings path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join("settings.json.tmp");
    let encoded =
        serde_json::to_vec_pretty(settings).map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(&temporary, encoded)?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_mock_preview_available_and_stat_toggles_independent() {
        let settings = AppSettings::default();
        assert!(settings.mock_mode);
        assert!(settings.auto_reconnect);
        assert_eq!(settings.nova_port, 9999);
        assert!(settings.toggles.cpu_temperature && settings.toggles.gpu_temperature);
        assert!(!settings.discord_application_id.contains("token"));
    }

    #[test]
    fn serialized_preferences_have_no_password_field() {
        let settings = AppSettings::default();
        let value = serde_json::to_value(settings).unwrap();
        assert!(value.get("password").is_none());
        assert!(value.get("nova_password").is_none());
    }
}
