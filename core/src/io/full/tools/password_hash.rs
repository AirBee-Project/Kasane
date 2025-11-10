use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use rand::rngs::OsRng;

pub fn hash_password(password: &str) -> Result<String, String> {
    todo!()
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
