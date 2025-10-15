use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString},
};
use rand::rngs::OsRng;

pub fn hash_password(password: &str) -> Result<String, String> {
    let argon2 = Argon2::default();

    // ランダムソルト生成
    let salt = SaltString::generate(&mut OsRng);

    // ハッシュを生成して PHC 形式の文字列にする
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt) // now works
        .map_err(|e| format!("error hashing password: {}", e))?;

    Ok(password_hash.to_string())
}

/// 入力パスワードが保存されたハッシュ（PHC 文字列）と一致するか検証
pub fn verify_password(password: &str, stored_phc: &str) -> Result<bool, String> {
    let argon2 = Argon2::default();
    let parsed_hash =
        PasswordHash::new(stored_phc).map_err(|e| format!("error parsing hash: {}", e))?;

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false), // パスワード不一致
        Err(e) => Err(format!("error verifying password: {}", e)), // その他エラー
    }
}
