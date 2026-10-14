/// Normalize Xbox 360 Title IDs without inferring a title from partial data.
pub fn normalize_title_id(title_id: &str) -> Option<String> {
    let value = title_id.trim();
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    (value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_ids_are_normalized_exactly() {
        assert_eq!(
            normalize_title_id(" 0x4d5307e6 ").as_deref(),
            Some("4D5307E6")
        );
        for invalid in ["4D5307E", "4D5307E6A", "4D53GGG6", "", "0x"] {
            assert_eq!(normalize_title_id(invalid), None, "{invalid}");
        }
    }
}
