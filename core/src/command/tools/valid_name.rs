pub fn valid_name(s: &str) -> bool {
    if s.len() > 256 {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
