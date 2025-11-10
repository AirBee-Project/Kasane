use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operation::kasane::{
    AppState, JWT_EXPIRATION_HOURS, JWT_EXPIRATION_MINUTES, JWT_SECRET, MAX_KEEPALIVE_SESSIONS,
};
use jsonwebtoken::{encode, EncodingKey, Header}; // ✅ 修正：jws::encodeを削除

// ==========================
// JWT クレーム構造体
// ==========================
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,        // ユーザー名
    pub session_id: String, // セッションID（ストレージで管理）
    pub exp: u64,           // 有効期限 (UNIX timestamp)
    pub iat: u64,           // 発行時刻 (UNIX timestamp)
    pub is_keepalive: bool, // Keep-alive対象かどうか
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

    // Keep-aliveセッション数をカウント
    let keepalive_count = state.storage.count_keepalive_sessions().unwrap_or(0);

    // 30ユーザーまでKeep-aliveを維持
    let is_keepalive = keepalive_count < MAX_KEEPALIVE_SESSIONS;

    // セッションIDを生成
    let session_id = Uuid::new_v4().to_string();

    // 現在時刻（UNIX秒）
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 有効期限を計算
    let expiration_secs = if is_keepalive {
        JWT_EXPIRATION_HOURS * 3600
    } else {
        JWT_EXPIRATION_MINUTES * 60
    };
    let expires_at = now_secs + expiration_secs;

    // ストレージにセッションを保存
    state
        .storage
        .create_session(&session_id, &req.username, expires_at, is_keepalive)
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
        is_keepalive,
    };

    // JWT トークンを生成 ✅（修正版）
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to generate token",
        )
    })?;

    // レスポンスを返す
    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer".to_string(),
        expires_in: expiration_secs,
    }))
}
