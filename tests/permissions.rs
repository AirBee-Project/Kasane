use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use kasane::{AppState, db_init, kasane};

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

        let app_state = AppState { db };
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
            r#"{{ "username": "{}", "password": "password", "privileges": {} }}"#,
            username,
            if is_global_admin {
                r#"[{ "scope": "global", "role": "admin" }]"#
            } else {
                "[]"
            }
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

/// 権限一覧を取得する。
async fn fetch_privileges(
    app: &axum::Router,
    token: &str,
    username: &str,
) -> Vec<serde_json::Value> {
    let req = Request::builder()
        .method("GET")
        .uri(format!("/users/{}/privileges", username))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["privileges"]
        .as_array()
        .unwrap()
        .clone()
}

/// 対象サブリソースのパスを組み立てる。
fn privilege_path(username: &str, db_name: Option<&str>, table_name: Option<&str>) -> String {
    match (db_name, table_name) {
        (None, _) => format!("/users/{}/privileges/global", username),
        (Some(db), None) => format!("/users/{}/privileges/databases/{}", username, db),
        (Some(db), Some(table)) => format!(
            "/users/{}/privileges/databases/{}/tables/{}",
            username, db, table
        ),
    }
}

/// 対象 1 件に権限を設定する（結果は呼び出し側が判断する）。
async fn put_privilege(
    app: &axum::Router,
    token: &str,
    username: &str,
    db_name: Option<&str>,
    table_name: Option<&str>,
    role: &str,
) -> axum::response::Response {
    let req = Request::builder()
        .method("PUT")
        .uri(privilege_path(username, db_name, table_name))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(format!(r#"{{"role": "{}"}}"#, role)))
        .unwrap();
    app.clone().oneshot(req).await.unwrap()
}

/// 対象 1 件の権限を剥奪する（結果は呼び出し側が判断する）。
async fn delete_privilege(
    app: &axum::Router,
    token: &str,
    username: &str,
    db_name: Option<&str>,
    table_name: Option<&str>,
) -> axum::response::Response {
    let req = Request::builder()
        .method("DELETE")
        .uri(privilege_path(username, db_name, table_name))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap()
}

async fn grant_privilege(
    app: &axum::Router,
    root_token: &str,
    username: &str,
    db_name: &str,
    role: &str,
) {
    let res = put_privilege(app, root_token, username, Some(db_name), None, role).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

async fn grant_table_privilege(
    app: &axum::Router,
    root_token: &str,
    username: &str,
    db_name: &str,
    table_name: &str,
    role: &str,
) {
    let res = put_privilege(
        app,
        root_token,
        username,
        Some(db_name),
        Some(table_name),
        role,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// レスポンスからステータスコードとエラーコードを抽出する。
async fn status_and_code(res: axum::response::Response) -> (StatusCode, String) {
    let status = res.status();
    let code = json_body(res).await["code"]
        .as_str()
        .unwrap_or("")
        .to_string();
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
        "manage",
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
    grant_privilege(&test_app.app, &root_token, "write_user", "test_db", "write").await;

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
    grant_privilege(&test_app.app, &root_token, "read_user", "test_db", "read").await;

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
    grant_privilege(&test_app.app, &root_token, "viewer", "visible_db", "read").await;

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

/// データベースのManage権限を持つユーザーが、他ユーザーへ権限を付与しようとすると拒否されることを検証する。
#[tokio::test]
async fn test_manage_user_can_set_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // 1. データベースを作成する
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    // 2. manage_user と normal_user を作成する
    let manage_token =
        create_user_and_token(&test_app.app, &root_token, "manage_user", false).await;
    let _normal_token =
        create_user_and_token(&test_app.app, &root_token, "normal_user", false).await;

    // 3. manage_user に Manage 権限を付与する（rootとして）
    grant_privilege(
        &test_app.app,
        &root_token,
        "manage_user",
        "test_db",
        "manage",
    )
    .await;

    // 4. manage_user が normal_user に Read 権限を付与しようとする（global の admin のみ可能なため失敗するはず）
    let res = put_privilege(
        &test_app.app,
        &manage_token,
        "normal_user",
        Some("test_db"),
        None,
        "read",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 5. normal_user が Read 権限を持っていないことを検証する
    let req = Request::builder()
        .method("GET")
        .uri("/users/normal_user")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["privileges"].as_array().unwrap().len(), 0);
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

#[tokio::test]
/// `/query` はクエリ式が参照する**すべてのデータベース**に Read 以上の権限を要求する。
/// 一部のソースにしか権限が無い場合は、他のソースにデータがあっても 403 で拒否されなければならない。
async fn test_query_authorization() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // Setup two DBs, each with a table and data, using root.
    for (db, table) in [("db_a", "t_a"), ("db_b", "t_b")] {
        let req = Request::builder()
            .method("POST")
            .uri("/databases")
            .header("Authorization", format!("Bearer {}", root_token))
            .header("Content-Type", "application/json")
            .body(Body::from(format!(r#"{{"name": "{}"}}"#, db)))
            .unwrap();
        assert_eq!(
            test_app.app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::CREATED
        );

        let req = Request::builder()
            .method("POST")
            .uri(format!("/databases/{}/tables", db))
            .header("Authorization", format!("Bearer {}", root_token))
            .header("Content-Type", "application/json")
            .body(Body::from(format!(
                r#"{{"name": "{}", "data_type": "Int", "max_zoom_level": 5}}"#,
                table
            )))
            .unwrap();
        assert_eq!(
            test_app.app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::CREATED
        );

        let req = Request::builder()
            .method("PUT")
            .uri(format!("/databases/{}/tables/{}/data", db, table))
            .header("Authorization", format!("Bearer {}", root_token))
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"value": 10, "spatial_ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }]}"#))
            .unwrap();
        assert_eq!(
            test_app.app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::OK
        );
    }

    // User only has Read on db_a.
    let user_token = create_user_and_token(&test_app.app, &root_token, "query_user", false).await;
    grant_privilege(&test_app.app, &root_token, "query_user", "db_a", "read").await;

    // Querying db_a alone is allowed.
    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{
            "spatial_ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }],
            "query": { "type": "source", "database": "db_a", "table": "t_a" }
        }"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // A query merging db_a (readable) with db_b (no privilege) must be rejected,
    // even though the db_a source alone would be allowed.
    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("Authorization", format!("Bearer {}", user_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{
            "spatial_ids": [{ "z": 0, "f": 0, "x": 0, "y": 0, "type": "singleId" }],
            "query": {
                "type": "merge",
                "left":  { "type": "source", "database": "db_a", "table": "t_a" },
                "right": { "type": "source", "database": "db_b", "table": "t_b" },
                "default": 0,
                "policy": "sum"
            }
        }"#,
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

    // root が管理者権限を剥奪 → DB 作成は Forbidden
    let res = delete_privilege(&test_app.app, &root_token, "demo_admin", None, None).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", admin_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "admin_db_fail"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 再ログインすると管理者ではなくなっている（DB 作成は Forbidden）
    create_user_and_token(&test_app.app, &root_token, "demo_admin2", true).await;
    let res = delete_privilege(&test_app.app, &root_token, "demo_admin2", None, None).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
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
    let res = delete_privilege(&test_app.app, &root_token, "root", None, None).await;
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
/// ログイン済みユーザーがサーバーのステータスとバージョン情報を取得できるか検証する。
/// 未ログインの場合は 401 (missing_token) で拒否されること。
async fn test_get_system_info() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // 1. 未ログイン（ヘッダーなし）→ 401 Unauthorized (missing_token)
    let req = Request::builder()
        .method("GET")
        .uri("/system/info")
        .body(Body::empty())
        .unwrap();
    let (status, code) = status_and_code(test_app.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code, "missing_token");

    // 2. ログイン済み（rootユーザー）→ 200 OK
    let req = Request::builder()
        .method("GET")
        .uri("/system/info")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
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
    assert!(json["privileges"].as_array().unwrap().is_empty());

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

#[tokio::test]
/// データベースのリネーム、コピー、およびテーブルコピーにおける権限検証をテストする。
async fn test_copy_and_rename_permissions() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // 1. データベース src_db を作成し、一般ユーザー user_a に Read 権限、user_b に Manage 権限を付与する。
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::from(r#"{"name": "src_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let user_a_token = create_user_and_token(&test_app.app, &root_token, "user_a", false).await;
    let user_b_token = create_user_and_token(&test_app.app, &root_token, "user_b", false).await;

    // user_a に Read 権限を付与
    grant_privilege(&test_app.app, &root_token, "user_a", "src_db", "read").await;

    // user_b に Manage 権限を付与
    grant_privilege(&test_app.app, &root_token, "user_b", "src_db", "manage").await;

    // 2. データベースのRename権限テスト
    // user_a (Read) はリネームできないはず (403)
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/src_db")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", user_a_token))
        .body(Body::from(r#"{"new_name": "renamed_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // user_b (Manage) はリネームできるはず (200)
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/src_db")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", user_b_token))
        .body(Body::from(r#"{"new_name": "renamed_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // renamed_db に名前が変更されたので、以降は renamed_db を対象とする。
    // user_a (Read) の権限が renamed_db に引き継がれていることを確認
    // user_a が renamed_db/copy に POST すると、Global Admin ではないので 403 Forbidden になる。
    let req = Request::builder()
        .method("POST")
        .uri("/databases/renamed_db/copy")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", user_a_token))
        .body(Body::from(r#"{"copy_name": "copied_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // root が renamed_db/copy に POST して成功させる。
    let req = Request::builder()
        .method("POST")
        .uri("/databases/renamed_db/copy")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::from(r#"{"copy_name": "copied_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // テスト継続のため root (Global Admin) が user_a に copied_db の Manage 権限を付与する。
    grant_privilege(&test_app.app, &root_token, "user_a", "copied_db", "manage").await;

    // 3. コピー先データベース (copied_db) に対する user_a の Manage 権限を検証
    // 手動で付与した Manage 権限により、user_a は copied_db にテーブルを作成できるはず (201)。
    // そのため、user_a は copied_db にテーブルを作成できるはず (201)。
    let req = Request::builder()
        .method("POST")
        .uri("/databases/copied_db/tables")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", user_a_token))
        .body(Body::from(
            r#"{"name": "new_table", "data_type": "Int", "max_zoom_level": 25}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 4. テーブルコピーの権限テスト
    // user_b (元src_dbのManageだったがリネームされてrenamed_dbのManage) は copied_db に対する権限を持たないため、
    // renamed_db から copied_db へのテーブルコピーは失敗するはず (copied_db の Manage 権限がないため 403)。
    let req = Request::builder()
        .method("POST")
        .uri("/databases/renamed_db/tables/new_table/copy")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", user_b_token))
        .body(Body::from(
            r#"{"copy_db_name": "copied_db", "copy_table_name": "copied_table"}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// 以下、階層スコープ権限（global / database / table）と楽観ロックの検証。
// ---------------------------------------------------------------------------

/// root 権限でデータベースを作る。
async fn create_db(app: &axum::Router, root_token: &str, name: &str) {
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(format!(r#"{{"name": "{}"}}"#, name)))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

/// テーブル作成を試みる（結果は呼び出し側が判断する）。
async fn post_table(
    app: &axum::Router,
    token: &str,
    db_name: &str,
    table_name: &str,
) -> axum::response::Response {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/databases/{}/tables", db_name))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"name": "{}", "data_type": "Int", "max_zoom_level": 5}}"#,
            table_name
        )))
        .unwrap();
    app.clone().oneshot(req).await.unwrap()
}

/// テーブルを作り、成功することを確かめる。
async fn create_table(app: &axum::Router, token: &str, db_name: &str, table_name: &str) {
    let res = post_table(app, token, db_name, table_name).await;
    assert_eq!(res.status(), StatusCode::CREATED);
}

async fn get(app: &axum::Router, token: &str, uri: &str) -> axum::response::Response {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap()
}

#[tokio::test]
/// データベースを削除して同じ名前で作り直しても、旧データベースへの権限が
/// 新しいデータベースに効かないことを検証する（権限は名前ではなく ID に紐づく）。
async fn test_privileges_do_not_survive_delete_and_recreate() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "target_db").await;
    let user_token = create_user_and_token(&test_app.app, &root_token, "grantee", false).await;
    grant_privilege(&test_app.app, &root_token, "grantee", "target_db", "manage").await;

    // 付与直後はアクセスできる
    let res = get(&test_app.app, &user_token, "/databases/target_db").await;
    assert_eq!(res.status(), StatusCode::OK);

    // データベースを削除し、同じ名前で作り直す
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/target_db")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    create_db(&test_app.app, &root_token, "target_db").await;

    // 旧権限は新しい target_db には効かない
    let res = get(&test_app.app, &user_token, "/databases/target_db").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 解決できなくなったルールは権限一覧にも現れない
    let privileges = fetch_privileges(&test_app.app, &root_token, "grantee").await;
    assert!(privileges.is_empty());
}

#[tokio::test]
/// データベースを改名しても権限が追従することを検証する。
async fn test_privileges_follow_database_rename() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "before_db").await;
    let user_token = create_user_and_token(&test_app.app, &root_token, "follower", false).await;
    grant_privilege(&test_app.app, &root_token, "follower", "before_db", "read").await;

    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/before_db")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"new_name": "after_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 改名先に権限が追従している
    let res = get(&test_app.app, &user_token, "/databases/after_db").await;
    assert_eq!(res.status(), StatusCode::OK);

    // 権限一覧の表示も新しい名前になっている
    let privileges = fetch_privileges(&test_app.app, &root_token, "follower").await;
    assert_eq!(privileges.len(), 1);
    assert_eq!(privileges[0]["db_name"], "after_db");
}

#[tokio::test]
/// 全データベースへの Manage（global/manage）と、サーバー管理者（global/admin）が
/// 別の権限であることを検証する。
async fn test_global_manage_is_not_server_admin() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "shared_db").await;
    let token = create_user_and_token(&test_app.app, &root_token, "data_manager", false).await;
    let res = put_privilege(
        &test_app.app,
        &root_token,
        "data_manager",
        None,
        None,
        "manage",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // データ面: 全データベースを Manage できる
    create_table(&test_app.app, &token, "shared_db", "managed").await;

    // 制御面: ユーザー一覧も、権限付与もできない
    let res = get(&test_app.app, &token, "/users").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = put_privilege(&test_app.app, &token, "data_manager", None, None, "admin").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
/// `admin` ロールは global スコープ以外では受け付けられないことを検証する。
///
/// データベース・テーブルスコープのリクエスト型は `admin` を持たない `DataRole` なので、
/// 実行時の検証ではなくデシリアライズの時点で弾かれる（他のスキーマ違反と同じ 422）。
async fn test_admin_role_is_rejected_outside_global_scope() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "scoped_db").await;
    create_table(&test_app.app, &root_token, "scoped_db", "scoped_table").await;
    create_user_and_token(&test_app.app, &root_token, "climber", false).await;

    for (db, table) in [
        (Some("scoped_db"), None),
        (Some("scoped_db"), Some("scoped_table")),
    ] {
        let res = put_privilege(&test_app.app, &root_token, "climber", db, table, "admin").await;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    // global スコープでだけ通る。
    let res = put_privilege(&test_app.app, &root_token, "climber", None, None, "admin").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // 何も保存されていないこと（データスコープ側）を確認する。
    let privileges = fetch_privileges(&test_app.app, &root_token, "climber").await;
    assert_eq!(privileges.len(), 1);
    assert_eq!(privileges[0]["scope"], "global");
}

#[tokio::test]
/// 存在しないデータベース・テーブルへの権限付与が拒否されることを検証する
/// （タイポの黙殺と、将来作られる同名オブジェクトへの事前付与の両方を防ぐ）。
async fn test_privileges_on_unknown_targets_are_rejected() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "real_db").await;
    create_user_and_token(&test_app.app, &root_token, "typo_user", false).await;

    let res = put_privilege(
        &test_app.app,
        &root_token,
        "typo_user",
        Some("raal_db"),
        None,
        "read",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["code"], "database_not_found");

    let res = put_privilege(
        &test_app.app,
        &root_token,
        "typo_user",
        Some("real_db"),
        Some("ghost"),
        "read",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(res).await["code"], "table_not_found");

    // 何も保存されていない
    assert!(
        fetch_privileges(&test_app.app, &root_token, "typo_user")
            .await
            .is_empty()
    );
}

#[tokio::test]
/// 同じ対象への再設定が「2 件目の追加」ではなく「置き換え」になることを検証する。
///
/// 対象がパスのキーなので、同一対象に複数のルールが並ぶこと自体が表現できない。
async fn test_setting_same_target_twice_replaces_the_rule() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "dup_db").await;
    create_user_and_token(&test_app.app, &root_token, "dup_user", false).await;

    grant_privilege(&test_app.app, &root_token, "dup_user", "dup_db", "manage").await;
    grant_privilege(&test_app.app, &root_token, "dup_user", "dup_db", "read").await;

    let privileges = fetch_privileges(&test_app.app, &root_token, "dup_user").await;
    assert_eq!(privileges.len(), 1);
    assert_eq!(privileges[0]["role"], "read");
}

#[tokio::test]
/// 同じ対象への再設定によるロールの降格が実際に効くことを検証する。
async fn test_role_downgrade_takes_effect() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "demote_db").await;
    let token = create_user_and_token(&test_app.app, &root_token, "demoted", false).await;
    grant_privilege(&test_app.app, &root_token, "demoted", "demote_db", "manage").await;

    // Manage のうちはテーブルを作れる
    create_table(&test_app.app, &token, "demote_db", "before_demote").await;

    // Read へ降格する
    grant_privilege(&test_app.app, &root_token, "demoted", "demote_db", "read").await;

    // 降格が効いており、テーブルを作れない
    let res = post_table(&test_app.app, &token, "demote_db", "after_demote").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 権限は 1 件だけで、ロールは read
    let privileges = fetch_privileges(&test_app.app, &root_token, "demoted").await;
    assert_eq!(privileges.len(), 1);
    assert_eq!(privileges[0]["role"], "read");
}

#[tokio::test]
/// 別々の対象に対する操作が互いに干渉しないことを検証する。
///
/// 権限セット全体を送る API では、古い一覧をもとにした付与が他者の剥奪を巻き戻し得た。
/// 対象ごとの操作なら、そもそも現在の一覧を知らずに書けるのでその事故が起きない。
async fn test_operations_on_distinct_targets_do_not_interfere() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "db_one").await;
    create_db(&test_app.app, &root_token, "db_two").await;
    create_user_and_token(&test_app.app, &root_token, "shared", false).await;

    grant_privilege(&test_app.app, &root_token, "shared", "db_one", "read").await;
    grant_privilege(&test_app.app, &root_token, "shared", "db_two", "manage").await;

    // db_two を剥奪したあとに db_one を触っても、剥奪は巻き戻らない。
    let res = delete_privilege(&test_app.app, &root_token, "shared", Some("db_two"), None).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    grant_privilege(&test_app.app, &root_token, "shared", "db_one", "manage").await;

    let privileges = fetch_privileges(&test_app.app, &root_token, "shared").await;
    assert_eq!(privileges.len(), 1);
    assert_eq!(privileges[0]["db_name"], "db_one");
    assert_eq!(privileges[0]["role"], "manage");
}

#[tokio::test]
/// 剥奪はロールを問わず対象ごと落ちること、権限が無ければ 404 になることを検証する。
async fn test_revoke_targets_the_object_not_the_role() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "rev_db").await;
    create_user_and_token(&test_app.app, &root_token, "revokee", false).await;

    grant_privilege(&test_app.app, &root_token, "revokee", "rev_db", "read").await;

    // ロールを指定しないので、Read だろうが Manage だろうが確実に落ちる。
    let res = delete_privilege(&test_app.app, &root_token, "revokee", Some("rev_db"), None).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    assert!(
        fetch_privileges(&test_app.app, &root_token, "revokee")
            .await
            .is_empty()
    );

    // 持っていない権限の剥奪は 404（黙って成功しない）。
    let res = delete_privilege(&test_app.app, &root_token, "revokee", Some("rev_db"), None).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// テーブルスコープの Manage が、そのテーブル以外を作る踏み台にならないことを検証する。
async fn test_table_scope_manage_cannot_create_other_tables() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "box_db").await;
    create_table(&test_app.app, &root_token, "box_db", "scratch").await;

    let token = create_user_and_token(&test_app.app, &root_token, "boxed", false).await;
    grant_table_privilege(
        &test_app.app,
        &root_token,
        "boxed",
        "box_db",
        "scratch",
        "manage",
    )
    .await;

    // 直接の新規テーブル作成はデータベースレベルの Manage が要るので拒否される
    let res = post_table(&test_app.app, &token, "box_db", "sneaked").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // コピー経由でも同じ。コピー先の権限判定にコピー元テーブル名を使ってはならない。
    let req = Request::builder()
        .method("POST")
        .uri("/databases/box_db/tables/scratch/copy")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"copy_table_name": "sneaked"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 自分のテーブルの管理（削除）はできる
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/box_db/tables/scratch")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
/// テーブルスコープの権限しか持たないユーザーが、自分のテーブルまで辿り着けることを検証する。
/// データベース一覧・テーブル一覧に現れ、かつ他のテーブルは見えない。
async fn test_table_scope_user_can_discover_own_table() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "visible_db").await;
    create_db(&test_app.app, &root_token, "hidden_db").await;
    create_table(&test_app.app, &root_token, "visible_db", "mine").await;
    create_table(&test_app.app, &root_token, "visible_db", "yours").await;

    let token = create_user_and_token(&test_app.app, &root_token, "narrow", false).await;
    grant_table_privilege(
        &test_app.app,
        &root_token,
        "narrow",
        "visible_db",
        "mine",
        "read",
    )
    .await;

    // データベース一覧には visible_db だけが出る
    let res = get(&test_app.app, &token, "/databases").await;
    assert_eq!(res.status(), StatusCode::OK);
    let dbs = json_body(res).await;
    let names: Vec<&str> = dbs
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["visible_db"]);

    // データベース情報も取得できる
    let res = get(&test_app.app, &token, "/databases/visible_db").await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = get(&test_app.app, &token, "/databases/hidden_db").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // テーブル一覧は自分が読めるものだけに絞られる
    let res = get(&test_app.app, &token, "/databases/visible_db/tables").await;
    assert_eq!(res.status(), StatusCode::OK);
    let tables = json_body(res).await;
    let table_names: Vec<&str> = tables
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(table_names, vec!["mine"]);

    // 権限のないテーブルの詳細は取得できない
    let res = get(&test_app.app, &token, "/databases/visible_db/tables/mine").await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = get(&test_app.app, &token, "/databases/visible_db/tables/yours").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
/// /query がテーブル単位で認可されることを検証する。
/// テーブルスコープの権限しか無くても、そのテーブルだけを参照するクエリは実行できる。
async fn test_query_authorization_is_table_scoped() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "q_db").await;
    create_table(&test_app.app, &root_token, "q_db", "allowed").await;
    create_table(&test_app.app, &root_token, "q_db", "denied").await;

    let token = create_user_and_token(&test_app.app, &root_token, "querier", false).await;
    grant_table_privilege(
        &test_app.app,
        &root_token,
        "querier",
        "q_db",
        "allowed",
        "read",
    )
    .await;

    let query = |table: &str| {
        serde_json::json!({
            "value_type": "Int",
            "spatial_ids": [{ "type": "rangeId", "z": 5, "f": [0, 0], "x": [0, 1], "y": [0, 1] }],
            "query": { "type": "source", "database": "q_db", "table": table }
        })
        .to_string()
    };

    // 権限のあるテーブルへのクエリは通る
    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(query("allowed")))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 権限のないテーブルは同じデータベース内でも拒否される
    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(query("denied")))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
/// 存在しないテーブルを参照したときの応答が、単一テーブル経路と /query で一致することを検証する。
///
/// データベースレベルの権限を持つユーザーには「権限がない」ではなく
/// 「テーブルが無い」と伝わらなければならない。
async fn test_missing_table_reports_not_found_consistently() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "consistent_db").await;
    let token = create_user_and_token(&test_app.app, &root_token, "dbmanager", false).await;
    grant_privilege(
        &test_app.app,
        &root_token,
        "dbmanager",
        "consistent_db",
        "manage",
    )
    .await;

    let query = serde_json::json!({
        "value_type": "Int",
        "spatial_ids": [{ "type": "rangeId", "z": 5, "f": [0, 0], "x": [0, 1], "y": [0, 1] }],
        "query": { "type": "source", "database": "consistent_db", "table": "ghost" }
    })
    .to_string();

    // 単一テーブル経路: テーブルが無いので 404
    let res = get(
        &test_app.app,
        &token,
        "/databases/consistent_db/tables/ghost",
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // /query 経路も同じ 404 でなければならない
    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(query.clone()))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // グローバル権限（root）でも 404
    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(query))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// 保存されている生の権限ルール数を数える。
///
/// `GET /users/{u}/privileges` は解決できないルールを隠すため、残留の有無は
/// API からは観測できない。ここではメタデータを直接読む。
fn stored_privilege_count(app_state: &AppState, username: &str) -> usize {
    use kasane::repositories::MetaRead;
    app_state
        .db
        .read(|repo| Ok(repo.require_user_meta(username)?.privileges.len()))
        .unwrap()
}

async fn delete_table(app: &axum::Router, token: &str, db_name: &str, table_name: &str) {
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/databases/{}/tables/{}", db_name, table_name))
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
/// 「テーブルの作成 → 権限付与 → 削除」を繰り返しても、削除済みリソースを指すルールが
/// ユーザーメタデータに累積しないことを検証する。
///
/// 権限の書き込みは常に全置換であり、置換時にすべてのルールが実在するリソースへ
/// 解決される必要があるため、書き込み直後の残留は必ず 0 件になる。よって残留は
/// 「直近の置換に含まれていたルールのうち、その後削除されたもの」に限られ、
/// 繰り返し回数に応じて増えることはない。
async fn test_stale_privileges_do_not_accumulate_over_cycles() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "churn_db").await;
    create_user_and_token(&test_app.app, &root_token, "churner", false).await;

    for i in 0..10 {
        let table = format!("tmp_{}", i);
        create_table(&test_app.app, &root_token, "churn_db", &table).await;
        grant_table_privilege(
            &test_app.app,
            &root_token,
            "churner",
            "churn_db",
            &table,
            "read",
        )
        .await;
        delete_table(&test_app.app, &root_token, "churn_db", &table).await;

        // 直前のサイクルで残留したルールは、次の全置換で必ず消える。
        // 残るのは常に「今回削除した 1 件」だけ。
        assert_eq!(
            stored_privilege_count(&test_app.app_state, "churner"),
            1,
            "サイクル {} で権限ルールが累積した",
            i
        );
    }

    // 解決できないルールは API 上には現れない。
    assert!(
        fetch_privileges(&test_app.app, &root_token, "churner")
            .await
            .is_empty()
    );
}

#[tokio::test]
/// `global` スコープの `read` が「全データベース・全テーブルを読めるが一切書けない」
/// 権限として機能することを検証する。
async fn test_global_read_can_read_everything_but_write_nothing() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "alpha_db").await;
    create_db(&test_app.app, &root_token, "beta_db").await;
    create_table(&test_app.app, &root_token, "alpha_db", "t_one").await;
    create_table(&test_app.app, &root_token, "beta_db", "t_two").await;

    let token = create_user_and_token(&test_app.app, &root_token, "reader", false).await;
    let res = put_privilege(&test_app.app, &root_token, "reader", None, None, "read").await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // データベースは全件見える
    let res = get(&test_app.app, &token, "/databases").await;
    assert_eq!(res.status(), StatusCode::OK);
    let names: Vec<String> = json_body(res)
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["alpha_db", "beta_db"]);

    // テーブルも全件見える（後から作られたデータベースにも及ぶ）
    create_db(&test_app.app, &root_token, "gamma_db").await;
    create_table(&test_app.app, &root_token, "gamma_db", "t_three").await;
    let res = get(&test_app.app, &token, "/databases/gamma_db/tables").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await.as_array().unwrap().len(), 1);

    // テーブル詳細もクエリも通る
    let res = get(&test_app.app, &token, "/databases/alpha_db/tables/t_one").await;
    assert_eq!(res.status(), StatusCode::OK);

    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "value_type": "Int",
                "spatial_ids": [{ "type": "rangeId", "z": 5, "f": [0, 0], "x": [0, 1], "y": [0, 1] }],
                "query": { "type": "source", "database": "beta_db", "table": "t_two" }
            })
            .to_string(),
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 書き込みはすべて拒否される
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/alpha_db/tables/t_one/data")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "spatial_ids": [{ "type": "singleId", "z": 5, "f": 0, "x": 0, "y": 0 }],
                "value": 1
            })
            .to_string(),
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // テーブル作成・削除も不可
    let res = post_table(&test_app.app, &token, "alpha_db", "nope").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/alpha_db/tables/t_one")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // データベース作成も、ユーザー管理も不可
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "nope_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let res = get(&test_app.app, &token, "/users").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// `table_id_index` に残っているテーブルの総数。到達不能になったテーブルの検出に使う。
fn indexed_table_count(app_state: &AppState) -> usize {
    app_state
        .db
        .read(|repo| Ok(repo.db.table_id_index.len(&repo.read_txn)? as usize))
        .unwrap()
}

#[tokio::test]
/// データベースを削除すると、配下のテーブルが 1 つ残らず消えることを検証する。
///
/// 列挙と削除が別トランザクションに分かれていると、その隙間に作られたテーブルが
/// 親を失って到達不能なまま残る。ここでは削除の完全性そのものを固定する。
async fn test_database_remove_leaves_no_orphan_tables() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    create_db(&test_app.app, &root_token, "doomed_db").await;
    create_db(&test_app.app, &root_token, "kept_db").await;
    for i in 0..3 {
        create_table(&test_app.app, &root_token, "doomed_db", &format!("t_{}", i)).await;
    }
    create_table(&test_app.app, &root_token, "kept_db", "survivor").await;
    assert_eq!(indexed_table_count(&test_app.app_state), 4);

    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/doomed_db")
        .header("Authorization", format!("Bearer {}", root_token))
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // 残るのは kept_db の 1 件だけ。doomed_db 配下は索引ごと消えている。
    assert_eq!(indexed_table_count(&test_app.app_state), 1);

    // 同名で作り直しても、以前のテーブルは見えない。
    create_db(&test_app.app, &root_token, "doomed_db").await;
    let res = get(&test_app.app, &root_token, "/databases/doomed_db/tables").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(json_body(res).await.as_array().unwrap().is_empty());
}

#[tokio::test]
/// 上限を超える権限ルールが、名前解決を走らせる前に件数だけで拒否されることを検証する。
async fn test_privilege_rules_are_capped_before_resolution() {
    let test_app = PermissionTestApp::new();
    let root_token = test_app.root_token();

    // 実在しないデータベースを指すルールを上限超えの件数だけ並べる。
    // 名前解決が先に走るなら database_not_found が返るはずだが、
    // 件数チェックが先なので invalid_privilege になる。
    let privileges: Vec<serde_json::Value> = (0..1001)
        .map(|i| {
            serde_json::json!({ "scope": "database", "db_name": format!("ghost_{}", i), "role": "read" })
        })
        .collect();

    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("Authorization", format!("Bearer {}", root_token))
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "hoarder",
                "password": "password",
                "privileges": privileges
            })
            .to_string(),
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(res).await["code"], "invalid_privilege");
}
