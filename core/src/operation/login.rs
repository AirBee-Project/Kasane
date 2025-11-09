/// POST /login - ログインエンドポイント
///
/// ⚠️ ユーザー認証のカスタマイズポイント ⚠️
pub async fn login_handler(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, &'static str)> {
    // ========================================
    // 👉 ここでユーザー名とパスワードを検証
    // ========================================
    if let Err(_) = state.storage.verify_user(&req.username, &req.password) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid username or password"));
    }
    // ========================================

    // 期限切れセッションをクリーンアップ
    let _ = state.storage.cleanup_expired_sessions();

    // Keep-aliveセッション数をカウント
    let keepalive_count = state.storage.count_keepalive_sessions().unwrap_or(0);

    // 30ユーザーまでKeep-aliveを維持
    let is_keepalive = keepalive_count < MAX_KEEPALIVE_SESSIONS;

    // セッションIDを生成
    let session_id = Uuid::new_v4().to_string();

    // 有効期限を計算
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

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

    // JWT トークンを生成
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

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer".to_string(),
        expires_in: expiration_secs,
    }))
}

// ==========================
// Storage の仮メソッド定義
// ==========================
// 以下のメソッドを Storage に実装する必要があります：
//
// impl Storage {
//     /// ユーザー認証
//     pub fn verify_user(&self, username: &str, password: &str) -> Result<(), UserError>;
//
//     /// セッションを作成（session_id, username, expires_at を保存）
//     pub fn create_session(&self, session_id: &str, username: &str, expires_at: u64, is_keepalive: bool) -> Result<(), UserError>;
//
//     /// セッションを検証（有効期限チェック＆期限切れは削除）
//     pub fn validate_session(&self, session_id: &str) -> Result<String, UserError>; // Ok(username)
//
//     /// Keep-aliveセッション数を取得
//     pub fn count_keepalive_sessions(&self) -> Result<usize, UserError>;
//
//     /// セッションを削除
//     pub fn delete_session(&self, session_id: &str) -> Result<(), UserError>;
//
//     /// 期限切れセッションをクリーンアップ
//     pub fn cleanup_expired_sessions(&self) -> Result<(), UserError>;
// }
