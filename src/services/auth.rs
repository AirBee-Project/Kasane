use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand_core::{OsRng, RngCore};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use redb::ReadableDatabase;

use crate::AppState;
use crate::error::{AppError, AuthError};
use crate::models::auth::Claims;
use crate::repositories::users::KasaneUsersRead;

/// プロセス全体で共有する JWT 署名鍵。
static JWT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();

pub fn jwt_secret() -> &'static [u8] {
    JWT_SECRET.get_or_init(
        || match std::env::var("JWT_SECRET").ok().filter(|s| !s.is_empty()) {
            Some(secret) => secret.into_bytes(),
            None => {
                let mut bytes = [0u8; 32];
                OsRng.fill_bytes(&mut bytes);
                tracing::warn!(
                    "JWT_SECRET is not set; using a randomly generated ephemeral secret. \
                     All issued tokens will be invalidated on restart. \
                     Set JWT_SECRET to a strong, unique value in production."
                );
                bytes.to_vec()
            }
        },
    )
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AppError::InternalError("Failed to hash password".to_string()))?
        .to_string();
    Ok(password_hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|_| AppError::InternalError("Invalid password hash format".to_string()))?;

    let is_valid = Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok();

    Ok(is_valid)
}

/// 存在しないユーザーへのログイン試行でも、実在ユーザーと同等の計算コストをかけるためのダミー検証。
pub fn dummy_verify_password(password: &str) {
    static DUMMY_HASH: OnceLock<String> = OnceLock::new();
    let dummy = DUMMY_HASH.get_or_init(|| {
        hash_password("dummy-password-for-constant-time-login")
            .expect("failed to build dummy password hash")
    });
    if let Ok(parsed) = PasswordHash::new(dummy) {
        let _ = Argon2::default().verify_password(password.as_bytes(), &parsed);
    }
}

/// 指定ユーザーの最新メタデータ（UUID・トークン世代）を読み込み、JWT を発行する。
pub fn generate_jwt(app_state: &AppState, username: &str) -> Result<String, AppError> {
    let meta = {
        let read_txn = app_state.redb.begin_read()?;
        let repo = KasaneUsersRead::new(read_txn);
        repo.get_user_meta(username)?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?
    };

    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
        + 24 * 3600;

    let claims = Claims {
        sub: username.to_string(),
        uid: meta.id.to_string(),
        ver: meta.token_version,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret()),
    )
    .map_err(|_| AppError::InternalError("Failed to generate token".to_string()))
}

pub fn verify_jwt(token: &str) -> Result<Claims, AppError> {
    let validation = Validation::default();
    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(jwt_secret()), &validation)
        .map_err(|_| AppError::Auth(AuthError::InvalidToken))?;

    Ok(token_data.claims)
}
