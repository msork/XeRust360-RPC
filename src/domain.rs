use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameIdentity {
    pub title_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AchievementProgress {
    pub unlocked: Option<u32>,
    pub total: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gamerscore {
    pub game: Option<u32>,
    pub total: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Temperatures {
    pub cpu_celsius: Option<f32>,
    pub gpu_celsius: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub game: GameIdentity,
    pub gamertag: Option<String>,
    pub achievements: AchievementProgress,
    pub gamerscore: Gamerscore,
    pub temperatures: Temperatures,
}

impl TelemetrySnapshot {
    pub fn has_running_game(&self) -> bool {
        self.game.title_id.is_some() || self.game.name.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StateToggles {
    pub gamertag: bool,
    pub achievements: bool,
    pub game_gamerscore: bool,
    pub total_gamerscore: bool,
    pub cpu_temperature: bool,
    pub gpu_temperature: bool,
}

impl Default for StateToggles {
    fn default() -> Self {
        Self {
            gamertag: true,
            achievements: true,
            game_gamerscore: true,
            total_gamerscore: true,
            cpu_temperature: true,
            gpu_temperature: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GameSession {
    current_title_id: Option<String>,
    started_at: Option<Instant>,
}

impl Default for GameSession {
    fn default() -> Self {
        Self::new()
    }
}

impl GameSession {
    pub fn new() -> Self {
        Self {
            current_title_id: None,
            started_at: None,
        }
    }

    /// Returns true when the observed title changes, including starting or stopping a title.
    pub fn observe_title(&mut self, title_id: Option<&str>, now: Instant) -> bool {
        if self.current_title_id.as_deref() == title_id {
            return false;
        }
        self.current_title_id = title_id.map(str::to_owned);
        self.started_at = title_id.map(|_| now);
        true
    }

    pub fn elapsed(&self, now: Instant) -> Option<Duration> {
        self.started_at
            .map(|started| now.saturating_duration_since(started))
    }
}
