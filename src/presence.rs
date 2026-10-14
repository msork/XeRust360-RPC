use std::time::Duration;

use crate::domain::{StateToggles, TelemetrySnapshot};

pub const ACTIVITY_NAME: &str = "Xbox 360";
pub const RPC_TEXT_LIMIT_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresencePayload {
    pub name: String,
    pub details: String,
    pub state: String,
    pub elapsed: Option<Duration>,
    pub large_image: String,
    pub small_image: String,
}

pub fn format_presence(
    snapshot: &TelemetrySnapshot,
    toggles: StateToggles,
    elapsed: Option<Duration>,
) -> Option<PresencePayload> {
    if !snapshot.has_running_game() {
        return None;
    }

    let details = snapshot.game.name.clone().or_else(|| {
        snapshot
            .game
            .title_id
            .as_ref()
            .map(|id| format!("Title ID {id}"))
    })?;

    let mut parts = Vec::new();
    if toggles.gamertag
        && let Some(value) = snapshot
            .gamertag
            .as_deref()
            .filter(|s| !s.trim().is_empty())
    {
        parts.push(value.to_owned());
    }
    if toggles.achievements
        && let (Some(unlocked), Some(total)) =
            (snapshot.achievements.unlocked, snapshot.achievements.total)
    {
        parts.push(format!("{unlocked}/{total} achievements"));
    }
    if toggles.game_gamerscore
        && let Some(score) = snapshot.gamerscore.game
    {
        parts.push(format!("{score} GS"));
    }
    if toggles.total_gamerscore
        && let Some(score) = snapshot.gamerscore.total
    {
        parts.push(format!("{score} GS"));
    }
    if toggles.cpu_temperature
        && let Some(temp) = snapshot.temperatures.cpu_celsius
    {
        parts.push(format_temperature("CPU", temp));
    }
    if toggles.gpu_temperature
        && let Some(temp) = snapshot.temperatures.gpu_celsius
    {
        parts.push(format_temperature("GPU", temp));
    }

    Some(PresencePayload {
        name: ACTIVITY_NAME.to_owned(),
        details: truncate_utf8(&details, RPC_TEXT_LIMIT_BYTES),
        state: truncate_utf8(&parts.join(" // "), RPC_TEXT_LIMIT_BYTES),
        elapsed,
        large_image: snapshot
            .game
            .cover_url
            .as_deref()
            .filter(|url| crate::artwork::valid_discord_image_url(url))
            .unwrap_or("xbox-360-placeholder")
            .to_owned(),
        small_image: "xerust360-rpc".to_owned(),
    })
}

fn format_temperature(component: &str, celsius: f32) -> String {
    if celsius.fract() == 0.0 {
        format!("{component} {:.0}°C", celsius)
    } else {
        format!("{component} {celsius:.1}°C")
    }
}

/// Truncate to a byte limit without splitting a UTF-8 codepoint.
pub fn truncate_utf8(value: &str, byte_limit: usize) -> String {
    if value.len() <= byte_limit {
        return value.to_owned();
    }
    let mut end = byte_limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain::GameIdentity, mock::sample_snapshot};

    #[test]
    fn formats_requested_state_in_order() {
        let presence = format_presence(&sample_snapshot(), StateToggles::default(), None).unwrap();
        assert_eq!(presence.name, "Xbox 360");
        assert_eq!(presence.details, "Halo 3");
        assert_eq!(presence.large_image, "xbox-360-placeholder");
        assert_eq!(
            presence.state,
            "MockPlayer // 12/49 achievements // 275 GS // 18250 GS // CPU 54°C // GPU 59°C"
        );
    }

    #[test]
    fn missing_values_are_omitted_and_game_name_falls_back_to_title_id() {
        let snapshot = TelemetrySnapshot {
            game: GameIdentity {
                title_id: Some("FFFE07D1".to_owned()),
                name: None,
                ..Default::default()
            },
            ..TelemetrySnapshot::default()
        };
        let presence = format_presence(&snapshot, StateToggles::default(), None).unwrap();
        assert_eq!(presence.details, "Title ID FFFE07D1");
        assert!(presence.state.is_empty());
        assert!(!presence.state.contains('0'));
    }

    #[test]
    fn toggles_remove_only_the_selected_components() {
        let toggles = StateToggles {
            gamertag: false,
            achievements: false,
            game_gamerscore: true,
            total_gamerscore: false,
            cpu_temperature: false,
            gpu_temperature: false,
        };
        let presence = format_presence(&sample_snapshot(), toggles, None).unwrap();
        assert_eq!(presence.state, "275 GS");
    }

    #[test]
    fn text_is_limited_without_splitting_unicode() {
        let input = "é".repeat(100);
        let output = truncate_utf8(&input, RPC_TEXT_LIMIT_BYTES);
        assert!(output.len() <= RPC_TEXT_LIMIT_BYTES);
        assert!(output.is_char_boundary(output.len()));
        assert_eq!(output, "é".repeat(64));
    }

    #[test]
    fn no_game_means_no_presence() {
        assert!(
            format_presence(&TelemetrySnapshot::default(), StateToggles::default(), None).is_none()
        );
    }

    #[test]
    fn external_https_cover_url_is_selected_and_invalid_url_uses_generic_asset() {
        let mut snapshot = sample_snapshot();
        snapshot.game.cover_url = Some("https://art.example/cover.jpg".to_owned());
        let presence = format_presence(&snapshot, StateToggles::default(), None).unwrap();
        assert_eq!(presence.large_image, "https://art.example/cover.jpg");

        snapshot.game.cover_url = Some("http://art.example/cover.jpg".to_owned());
        let presence = format_presence(&snapshot, StateToggles::default(), None).unwrap();
        assert_eq!(presence.large_image, "xbox-360-placeholder");
    }
}

#[cfg(test)]
mod timer_tests {
    use std::time::{Duration, Instant};

    use crate::domain::GameSession;

    #[test]
    fn timer_starts_and_resets_on_title_change() {
        let start = Instant::now();
        let mut session = GameSession::new();
        assert!(session.observe_title(Some("00000001"), start));
        assert_eq!(
            session.elapsed(start + Duration::from_secs(5)),
            Some(Duration::from_secs(5))
        );
        assert!(session.observe_title(Some("00000002"), start + Duration::from_secs(5)));
        assert_eq!(
            session.elapsed(start + Duration::from_secs(8)),
            Some(Duration::from_secs(3))
        );
        assert!(session.observe_title(None, start + Duration::from_secs(9)));
        assert_eq!(session.elapsed(start + Duration::from_secs(10)), None);
    }
}
