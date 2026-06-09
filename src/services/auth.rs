use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand_core::OsRng;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::AppError;
use crate::models::auth::Claims;

// In a real app, this should be in an environment variable
pub fn jwt_secret() -> Vec<u8> {
    std::env::var("KASANE_JWT_SECRET")
        .unwrap_or_else(|_| "kasane-super-secret-key-change-me".to_string())
        .into_bytes()
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

pub fn generate_jwt(username: &str) -> Result<String, AppError> {
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
        + 24 * 3600; // 24 hours

    let claims = Claims {
        sub: username.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&jwt_secret()),
    )
    .map_err(|_| AppError::InternalError("Failed to generate token".to_string()))
}

pub fn verify_jwt(token: &str) -> Result<Claims, AppError> {
    let validation = Validation::default();
    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(&jwt_secret()), &validation)
        .map_err(|_| AppError::Unauthorized("Invalid token".to_string()))?;

    Ok(token_data.claims)
}
