use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use kasane::{AppState, db_init, kasane};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tower::ServiceExt;

pub struct PermissionTestApp {
    pub app: axum::Router,
    pub app_state: AppState,
    _temp_file: NamedTempFile,
}

impl PermissionTestApp {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = db_init::initialize_database(temp_file.path().to_str().unwrap());

        let app_state = AppState {
            redb: Arc::new(db),
            auth_cache: Arc::new(kasane::auth_cache::AuthCache::new()),
        };
        // We DO NOT inject the root token automatically here.
        // The tests will need to explicitly send the Authorization header.
        let app = kasane(app_state.clone());

        Self {
            app,
            app_state,
            _temp_file: temp_file,
        }
    }

    /// 現在の DB 状態（root の uid・トークン世代）に基づく有効な root トークンを発行する。
    fn root_token(&self) -> String {
        kasane::services::auth::generate_jwt(&self.app_state, "root").unwrap()
    }
}

async fn create_user_and_token(
    app: &axum::Router,
    root_token: &str,
    username: &str,
    is_global_admin: bool,
) -> String {
    // 1. Create User
    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"username": "{}", "password": "password", "is_global_admin": {}}}"#,
            username, is_global_admin
        )))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. Login to get token
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"username": "{}", "password": "password"}}"#,
            username
        )))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    json["token"].as_str().unwrap().to_string()
}

async fn grant_privilege(
    app: &axum::Router,
    root_token: &str,
    username: &str,
    db_name: &str,
    role: &str,
) {
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/users/{}/privileges/{}", username, db_name))
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(format!(r#"{{"role": "{}"}}"#, role)))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

/// レスポンスの (status, error code) を取り出す
async fn status_and_code(res: axum::response::Response) -> (StatusCode, String) {
    let status = res.status();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let code = json["code"].as_str().unwrap_or("").to_string();
    (status, code)
}

#[tokio::test]
async fn test_auth_error_codes_are_structured() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // DB を1つ作っておく（権限不足コードの確認用）
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "code_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    // 一般ユーザー（権限なし）
    let user_token = create_user_and_token(&test_app.app, &root_token, "code_user", false).await;

    // 1. ヘッダ無し → missing_token
    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .body(Body::empty())
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code, "missing_token");

    // 2. Bearer でない → malformed_header
    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .header("Authorization", "Basic abc")
        .body(Body::empty())
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code, "malformed_header");

    // 3. 不正なトークン → invalid_token
    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .header("Authorization", "Bearer not-a-jwt")
        .body(Body::empty())
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code, "invalid_token");

    // 4. ログイン失敗 → invalid_credentials
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"username": "code_user", "password": "wrong"}"#,
        ))
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code, "invalid_credentials");

    // 5. GlobalAdmin 専用エンドポイント → requires_global_admin
    let req = Request::builder()
        .method("GET")
        .uri("/users")
        .header("Authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(code, "requires_global_admin");

    // 6. DB 権限不足 → insufficient_privilege
    let req = Request::builder()
        .method("GET")
        .uri("/databases/code_db")
        .header("Authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(code, "insufficient_privilege");

    // 7. root の削除 → root_protected
    let req = Request::builder()
        .method("DELETE")
        .uri("/users/root")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(code, "root_protected");
}

#[tokio::test]
async fn test_global_admin_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    let admin_token = create_user_and_token(&test_app.app, &root_token, "admin_user", true).await;

    // Admin should be able to create a DB
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", admin_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "admin_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_manage_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // Create a DB using root
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let user_token = create_user_and_token(&test_app.app, &root_token, "manage_user", false).await;
    grant_privilege(
        &test_app.app,
        &root_token,
        "manage_user",
        "test_db",
        "Manage",
    )
    .await;

    // Manage user cannot create another DB
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db_2"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Manage user CAN create a table in their DB
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "t1", "data_type": "Int", "max_zoom_level": 5}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_write_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // Create DB & Table using root
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "t1", "data_type": "Int", "max_zoom_level": 5}"#,
        ))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let user_token = create_user_and_token(&test_app.app, &root_token, "write_user", false).await;
    grant_privilege(&test_app.app, &root_token, "write_user", "test_db", "Write").await;

    // Write user CANNOT create a table
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "t2", "data_type": "Int", "max_zoom_level": 5}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Write user CAN insert data
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/t1/data")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"value": 10, "query": { "ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }], "type": "spatialIds" }}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_read_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // Setup DB, Table and Data with root
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "t1", "data_type": "Int", "max_zoom_level": 5}"#,
        ))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/t1/data")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"value": 10, "query": { "ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }], "type": "spatialIds" }}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let user_token = create_user_and_token(&test_app.app, &root_token, "read_user", false).await;
    grant_privilege(&test_app.app, &root_token, "read_user", "test_db", "Read").await;

    // Read user CANNOT insert data
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/t1/data")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"value": 20, "query": { "ids": [{ "z": 1, "f": 0, "x": 0, "y": 0, "type": "singleId" }], "type": "spatialIds" }}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Read user CAN get data
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables/t1/data/search")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"query": { "ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }], "type": "spatialIds" }}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_database_list_and_info_authorization() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // root creates two databases
    for name in ["visible_db", "hidden_db"] {
        let req = Request::builder()
            .method("POST")
            .uri("/databases")
            .header("Authorization", format!("Bearer {}", root_token))
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"name": "{}"}}"#, name)))
            .unwrap();
        let res = test_app.app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    // A non-admin user with Read on visible_db only
    let user_token = create_user_and_token(&test_app.app, &root_token, "viewer", false).await;
    grant_privilege(&test_app.app, &root_token, "viewer", "visible_db", "Read").await;

    // GET /databases returns only the database the user can access
    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let names: Vec<String> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["visible_db".to_string()]);

    // GET /databases/{name} allowed for the database the user can read
    let req = Request::builder()
        .method("GET")
        .uri("/databases/visible_db")
        .header("Authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // GET /databases/{name} forbidden for a database the user has no privilege on
    let req = Request::builder()
        .method("GET")
        .uri("/databases/hidden_db")
        .header("Authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // GlobalAdmin (root) sees all databases
    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_no_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // Setup DB, Table with root
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "t1", "data_type": "Int", "max_zoom_level": 5}"#,
        ))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let user_token = create_user_and_token(&test_app.app, &root_token, "no_user", false).await;

    // No user CANNOT get data
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables/t1/data/search")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"query": { "ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }], "type": "spatialIds" }}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// 指定ユーザーが /databases にアクセスできるか（=トークンが有効か）を返す
async fn token_is_valid(app: &axum::Router, token: &str) -> bool {
    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    res.status() == StatusCode::OK
}

#[tokio::test]
async fn test_password_change_revokes_tokens() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    let user_token = create_user_and_token(&test_app.app, &root_token, "rotate_user", false).await;
    assert!(token_is_valid(&test_app.app, &user_token).await);

    // root がパスワードを変更すると、既存トークンは失効する
    let req = Request::builder()
        .method("PUT")
        .uri("/users/rotate_user/password")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"password": "newpassword"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    assert!(!token_is_valid(&test_app.app, &user_token).await);
}

#[tokio::test]
async fn test_admin_demotion_and_root_protection() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // 管理者ユーザーを作成
    let admin_token = create_user_and_token(&test_app.app, &root_token, "demo_admin", true).await;

    // 管理者は DB を作成できる
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", admin_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "admin_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // root が管理者権限を剥奪 → 既存トークンは失効する
    let req = Request::builder()
        .method("PUT")
        .uri("/users/demo_admin/admin")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"is_global_admin": false}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(!token_is_valid(&test_app.app, &admin_token).await);

    // 再ログインすると管理者ではなくなっている（DB 作成は Forbidden）
    create_user_and_token(&test_app.app, &root_token, "demo_admin2", true).await;
    let req = Request::builder()
        .method("PUT")
        .uri("/users/demo_admin2/admin")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"is_global_admin": false}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();
    // 再ログイン
    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"username": "demo_admin2", "password": "password"}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let new_token = json["token"].as_str().unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", new_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "should_fail"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // root の管理者権限は変更できない
    let req = Request::builder()
        .method("PUT")
        .uri("/users/root/admin")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"is_global_admin": false}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_username_reuse_rejects_old_token() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // ユーザーを作成しトークンを取得
    let old_token = create_user_and_token(&test_app.app, &root_token, "reuse_user", false).await;
    assert!(token_is_valid(&test_app.app, &old_token).await);

    // ユーザーを削除
    let req = Request::builder()
        .method("DELETE")
        .uri("/users/reuse_user")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(!token_is_valid(&test_app.app, &old_token).await);

    // 同名ユーザーを再作成 → 旧トークンは別 UUID のため無効のまま
    create_user_and_token(&test_app.app, &root_token, "reuse_user", false).await;
    assert!(!token_is_valid(&test_app.app, &old_token).await);
}
