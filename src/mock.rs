use crate::domain::{
    AchievementProgress, GameIdentity, Gamerscore, TelemetrySnapshot, Temperatures,
};

/// Explicitly synthetic data for preview and development. Never selected as live telemetry.
pub fn sample_snapshot() -> TelemetrySnapshot {
    TelemetrySnapshot {
        game: GameIdentity {
            title_id: Some("4D5307E6".to_owned()),
            name: Some("Halo 3".to_owned()),
            ..Default::default()
        },
        gamertag: Some("MockPlayer".to_owned()),
        achievements: AchievementProgress {
            unlocked: Some(12),
            total: Some(49),
        },
        gamerscore: Gamerscore {
            game: Some(275),
            total: Some(18250),
        },
        temperatures: Temperatures {
            cpu_celsius: Some(54.0),
            gpu_celsius: Some(59.0),
        },
    }
}
