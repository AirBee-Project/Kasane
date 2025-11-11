use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operation::kasane::AppState;
use jsonwebtoken::{encode, EncodingKey, Header};

// ==========================
// JWT クレーム構造体
// ==========================
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,        // ユーザー名
    pub session_id: String, // セッションID（ストレージで管理）
    pub exp: u64,           // 有効期限 (UNIX timestamp)
    pub iat: u64,           // 発行時刻 (UNIX timestamp)
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
    token_type: String,
    expires_in: u64,
}

/// POST /login - ログインエンドポイント
///
/// ⚠️ ユーザー認証のカスタマイズポイント ⚠️
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
    // ユーザー認証
    if let Err(_) = state.storage.verify_user(&req.username, &req.password) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid username or password"));
    }

    // 古いセッションをクリーンアップ
    let _ = state.storage.cleanup_expired_sessions();

    // セッションIDを生成
    let session_id = Uuid::new_v4().to_string();

    // 現在時刻（UNIX秒）
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 有効期限を計算
    let expiration_secs = (state.conf.general.jwt_expiration_minutes * 60) as u64;
    let expires_at = now_secs + expiration_secs;

    // ストレージにセッションを保存
    state
        .storage
        .create_session(&session_id, &req.username, expiration_secs)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create session",
            )
        })?;

    // JWT クレームを作成
    let claims = Claims {
        sub: req.username.clone(),
        session_id: session_id.clone(),
        exp: expires_at,
        iat: now_secs,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(
            dotenvy::var("JWT_SECRET")
                .expect("JWT_SECRET must be set")
                .as_bytes(),
        ),
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to generate token",
        )
    })?;

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer".to_string(),
        expires_in: expiration_secs,
    }))
}
