//! パスワードのハッシュと JWT。インメモリ（root 固定）では使わない。

use crate::repositories::{CatalogRepository, Storage};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use base64ct::{Base64UrlUnpadded, Encoding};
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;
use crate::error::{AppError, AuthError, Resource};
use crate::models::auth::Claims;

/// プロセス全体で共有する JWT 署名鍵。
static JWT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();

#[tracing::instrument(skip_all)]
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

#[tracing::instrument(skip_all)]
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal("Failed to hash password".to_string()))?
        .to_string();
    Ok(password_hash)
}

#[tracing::instrument(skip_all)]
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|_| AppError::Internal("Invalid password hash format".to_string()))?;

    let is_valid = Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok();

    Ok(is_valid)
}

/// 存在しないユーザーへのログイン試行でも、実在ユーザーと同等の計算コストをかけるためのダミー検証。
#[tracing::instrument(skip_all)]
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
#[tracing::instrument(skip_all, fields(username = %username))]
pub async fn generate_jwt(app_state: &AppState, username: &str) -> Result<String, AppError> {
    let owned = username.to_string();
    let meta = app_state
        .db
        .read(async move |repo| repo.user_record(&owned).await)
        .await?
        .ok_or_else(|| Resource::User.not_found(username))?;

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

    encode_jwt(&claims)
}

fn encode_jwt(claims: &Claims) -> Result<String, AppError> {
    let payload = serde_json::to_vec(claims)
        .map_err(|e| AppError::Internal(format!("Failed to generate token: {e}")))?;
    let signing_input = format!(
        "{JWT_HEADER}.{}",
        Base64UrlUnpadded::encode_string(&payload)
    );
    let signature = jwt_mac(&signing_input).finalize().into_bytes();
    Ok(format!(
        "{signing_input}.{}",
        Base64UrlUnpadded::encode_string(&signature)
    ))
}

/// `{"typ":"JWT","alg":"HS256"}` の base64url。発行するトークンのヘッダは常にこれなので、
/// 検証でも完全一致だけを受け付け、`alg` の差し替え（`none` など）を構造的に防ぐ。
const JWT_HEADER: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9";

/// 時計のずれを許す秒数。
const JWT_LEEWAY_SECS: u64 = 60;

fn jwt_mac(signing_input: &str) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(jwt_secret()).expect("HMAC accepts any key length");
    mac.update(signing_input.as_bytes());
    mac
}

#[tracing::instrument(skip_all)]
pub fn verify_jwt(token: &str) -> Result<Claims, AppError> {
    let invalid = |reason: &str| {
        tracing::warn!("verify_jwt failed: {reason}");
        AppError::Auth(AuthError::InvalidToken("invalid or expired token"))
    };

    let (signing_input, signature) = token.rsplit_once('.').ok_or_else(|| invalid("malformed"))?;
    let (header, payload) = signing_input
        .split_once('.')
        .ok_or_else(|| invalid("malformed"))?;
    if header != JWT_HEADER {
        return Err(invalid("unexpected header"));
    }
    let signature =
        Base64UrlUnpadded::decode_vec(signature).map_err(|_| invalid("malformed signature"))?;
    jwt_mac(signing_input)
        .verify_slice(&signature)
        .map_err(|_| invalid("bad signature"))?;

    let payload =
        Base64UrlUnpadded::decode_vec(payload).map_err(|_| invalid("malformed payload"))?;
    let claims: Claims =
        serde_json::from_slice(&payload).map_err(|_| invalid("malformed claims"))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if (claims.exp as u64).saturating_add(JWT_LEEWAY_SECS) <= now {
        return Err(invalid("expired"));
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(exp: usize) -> Claims {
        Claims {
            sub: "alice".to_string(),
            uid: "uid".to_string(),
            ver: 1,
            exp,
        }
    }

    fn now() -> usize {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
    }

    /// 発行したトークンは検証を通り、中身がそのまま戻る。
    #[test]
    fn issued_token_round_trips() {
        let token = encode_jwt(&claims(now() + 3600)).unwrap();
        let verified = verify_jwt(&token).unwrap();
        assert_eq!(verified.sub, "alice");
        assert_eq!(verified.ver, 1);
    }

    /// 署名・ペイロード・ヘッダのどれを書き換えても通らない。
    #[test]
    fn tampered_tokens_are_rejected() {
        let token = encode_jwt(&claims(now() + 3600)).unwrap();
        let (signing_input, _) = token.rsplit_once('.').unwrap();

        let forged_signature = format!("{signing_input}.AAAA");
        assert!(verify_jwt(&forged_signature).is_err());

        let other = encode_jwt(&Claims {
            sub: "mallory".to_string(),
            ..claims(now() + 3600)
        })
        .unwrap();
        let other_payload = other.split('.').nth(1).unwrap();
        let (_, signature) = token.rsplit_once('.').unwrap();
        let swapped_payload = format!("{JWT_HEADER}.{other_payload}.{signature}");
        assert!(verify_jwt(&swapped_payload).is_err());

        // {"alg":"none","typ":"JWT"}
        let (_, after_header) = token.split_once('.').unwrap();
        let alg_none = format!("eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.{after_header}");
        assert!(verify_jwt(&alg_none).is_err());
    }

    /// 失効時刻を猶予（60 秒）以上過ぎたトークンは通らない。
    #[test]
    fn expired_tokens_are_rejected() {
        let expired = encode_jwt(&claims(now() - 120)).unwrap();
        assert!(verify_jwt(&expired).is_err());

        let within_leeway = encode_jwt(&claims(now() - 10)).unwrap();
        assert!(verify_jwt(&within_leeway).is_ok());
    }
}
