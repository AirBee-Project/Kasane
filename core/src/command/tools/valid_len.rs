pub fn valid_len(s: &str) -> bool {
    if s.len() > 256 {
        return false;
    };
    true
}
