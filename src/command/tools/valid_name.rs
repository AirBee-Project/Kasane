pub fn valid_name(s: &str) -> Result<(), &'static str> {
    let error_message = "only a-z, A-Z, 0-9, - _ . allowed, max 256 characters";

    //長さに関するチェック
    if s.len() > 256 {
        return Err(error_message);
    }

    //含まれる文字列に関するチェック
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Ok(())
    } else {
        Err(error_message)
    }
}
