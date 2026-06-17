use crate::error::AppError;

/// Kasane内部のエンティティ名の命名規則チェック
/// DatabaseやTableやUserの名前に使用する
///  
/// - 空文字禁止
/// - 1〜64文字
/// - 英数字 + `_` のみ
/// - 先頭は英字
/// - 予約語禁止
pub fn name_valid(name: &str) -> Result<(), AppError> {
    const MAX_LEN: usize = 64;

    const RESERVED: &[&str] = &[
        "system",
        "admin",
        "root",
        "database",
        "databases",
        "table",
        "tables",
        "index",
        "meta",
        "_internal",
    ];

    // 空文字
    if name.is_empty() {
        return Err(AppError::InvalidName {
            reason: "name is empty".to_string(),
        });
    }

    // 長さ
    if name.len() > MAX_LEN {
        return Err(AppError::InvalidName {
            reason: format!("name is too long (max: {})", MAX_LEN),
        });
    }

    // 先頭文字
    let first = name.chars().next().unwrap();

    if !first.is_ascii_alphabetic() {
        return Err(AppError::InvalidName {
            reason: "name must start with alphabetic character".to_string(),
        });
    }

    // 使用可能文字
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::InvalidName {
            reason: "name may only contain [a-zA-Z0-9_]".to_string(),
        });
    }

    // 予約語
    if RESERVED.contains(&name.to_lowercase().as_str()) {
        return Err(AppError::InvalidName {
            reason: format!("'{}' is reserved", name),
        });
    }

    Ok(())
}
