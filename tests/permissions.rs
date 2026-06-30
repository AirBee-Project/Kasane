use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use kasane::{AppState, db_init, kasane};
use std::sync::Arc;
use tower::ServiceExt;

pub struct PermissionTestApp {
    pub app: axum::Router,
    pub app_state: AppState,
    _temp_dir: tempfile::TempDir,
}

impl PermissionTestApp {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db = db_init::initialize_database(temp_dir.path().to_str().unwrap());

        let app_state = AppState {
            db: db.clone(),
            auth_cache: Arc::new(kasane::auth_cache::AuthCache::new()),
        };
        // We DO NOT inject the root token automatically here.
        // The tests will need to explicitly send the Authorization header.
        let app = kasane(app_state.clone());

        Self {
            app,
            app_state,
            _temp_dir: temp_dir,
        }
    }

    /// 現在のDB状態に基づく有効なrootトークンを発行する。
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

/// レスポンスからステータスコードとエラーコードを抽出する。
async fn status_and_code(res: axum::response::Response) -> (StatusCode, String) {
    let status = res.status();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let code = json["code"].as_str().unwrap_or("").to_string();
    (status, code)
}

#[tokio::test]
/// 認証・認可に関する各種エラーコードが正しく構造化されて返されるかを検証する。
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
/// Global Adminがデータベースを作成できるかを検証する。
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
/// Manage権限を持つユーザーがDB作成はできず、テーブル作成はできるかを検証する。
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
/// Write権限を持つユーザーがテーブル作成はできず、データ挿入はできるかを検証する。
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
        .body(Body::from(r#"{"value": 10, "spatial_ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }]}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
/// Read権限を持つユーザーがデータ挿入はできず、データ取得はできるかを検証する。
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
        .body(Body::from(r#"{"value": 10, "spatial_ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }]}"#))
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
        .body(Body::from(r#"{"value": 20, "spatial_ids": [{ "z": 1, "f": 0, "x": 0, "y": 0, "type": "singleId" }]}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Read user CAN get data
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables/t1/data/search")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"spatial_ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }]}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
/// データベース一覧および詳細取得が、ユーザーの権限に応じて正しくフィルタリングされるかを検証する。
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
async fn test_manage_user_can_set_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // 1. Create a DB
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    // 2. Create manage_user and normal_user
    let manage_token =
        create_user_and_token(&test_app.app, &root_token, "manage_user", false).await;
    let _normal_token =
        create_user_and_token(&test_app.app, &root_token, "normal_user", false).await;

    // 3. Grant Manage privilege to manage_user (as root)
    let req = Request::builder()
        .method("PUT")
        .uri("/users/manage_user/privileges/test_db")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::from(r#"{"role": "Manage"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // 4. manage_user tries to grant Read privilege to normal_user (should fail, only global admin can)
    let req = Request::builder()
        .method("PUT")
        .uri("/users/normal_user/privileges/test_db")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", manage_token))
        .body(Body::from(r#"{"role": "Read"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 5. verify normal_user does NOT have Read privilege
    let req = Request::builder()
        .method("GET")
        .uri("/users/normal_user/privileges")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
/// 権限を持たないユーザーがデータベース内のデータにアクセスできないかを検証する。
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
        .body(Body::from(
            r#"{"spatial_ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }]}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// 指定ユーザーのトークンが有効か（データベース一覧を取得できるか）を検証する。
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
/// パスワード変更時に、既存のセッション（トークン）が失効し再ログインが要求されるかを検証する。
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
/// 管理者権限の剥奪時にトークンが失効し、rootの権限は変更できないことを検証する。
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
/// ユーザー削除後に同名ユーザーを再作成した場合、旧ユーザーのトークンが無効になるかを検証する。
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

#[tokio::test]
/// GET /users/{username} のエンドポイントが、本人またはGlobal Adminのみに許可されているかを検証する。
async fn test_get_user_info_authorization() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // ユーザーA（一般ユーザー）を作成
    let user_a_token = create_user_and_token(&test_app.app, &root_token, "user_a", false).await;
    // ユーザーB（一般ユーザー）を作成
    let user_b_token = create_user_and_token(&test_app.app, &root_token, "user_b", false).await;
    // ユーザーC（管理者）を作成
    let admin_token = create_user_and_token(&test_app.app, &root_token, "admin_user", true).await;

    // 1. 本人が自分自身の情報を取得できるか (200 OK)
    let req = Request::builder()
        .method("GET")
        .uri("/users/user_a")
        .header("Authorization", format!("Bearer {}", user_a_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["username"], "user_a");
    assert_eq!(json["is_global_admin"], false);

    // 2. 他人（非管理者）が情報を取得しようとすると失敗するか (403 Forbidden)
    let req = Request::builder()
        .method("GET")
        .uri("/users/user_a")
        .header("Authorization", format!("Bearer {}", user_b_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. 管理者が他人の情報を取得できるか (200 OK)
    let req = Request::builder()
        .method("GET")
        .uri("/users/user_a")
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. 存在しないユーザーの情報を取得しようとすると失敗するか (404 Not Found)
    let req = Request::builder()
        .method("GET")
        .uri("/users/non_existent_user")
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
