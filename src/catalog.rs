/// Small offline fallback for well-known titles. Unknown identifiers remain unresolved.
pub fn resolve_title_name(title_id: &str) -> Option<&'static str> {
    let trimmed = title_id.trim();
    let normalized = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    match normalized.as_str() {
        "4D5307E6" => Some("Halo 3"),
        "4D53085B" => Some("Halo: Reach"),
        "4D530808" => Some("Halo Wars"),
        "4D530919" => Some("Halo 4"),
        "545408A7" => Some("Grand Theft Auto V"),
        "425307E6" => Some("The Elder Scrolls V: Skyrim"),
        _ => None,
    }
}

/// The returned Discord key must be uploaded as a Rich Presence asset in the chosen app.
pub fn resolve_artwork_key(title_id: &str) -> Option<&'static str> {
    let trimmed = title_id.trim();
    let normalized = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    match normalized.as_str() {
        "4D5307E6" => Some("halo-3"),
        "4D53085B" => Some("halo-reach"),
        "4D530808" => Some("halo-wars"),
        "4D530919" => Some("halo-4"),
        "545408A7" => Some("grand-theft-auto-v"),
        "425307E6" => Some("skyrim"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_id_resolution_accepts_nova_hex_forms_and_unknowns_stay_unknown() {
        assert_eq!(resolve_title_name("0x4d5307e6"), Some("Halo 3"));
        assert_eq!(resolve_title_name("545408A7"), Some("Grand Theft Auto V"));
        assert_eq!(resolve_title_name("DEADBEEF"), None);
        assert_eq!(resolve_artwork_key("4D5307E6"), Some("halo-3"));
        assert_eq!(resolve_artwork_key("DEADBEEF"), None);
    }
}
