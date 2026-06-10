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
    _temp_file: NamedTempFile,
}

impl PermissionTestApp {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = db_init::initialize_database(temp_file.path().to_str().unwrap());

        let app_state = AppState {
            redb: Arc::new(db),
            auth_cache: Arc::new(tokio::sync::RwLock::new(
                kasane::auth_cache::AuthCache::new(),
            )),
        };
        // We DO NOT inject the root token automatically here.
        // The tests will need to explicitly send the Authorization header.
        let app = kasane(app_state);

        Self {
            app,
            _temp_file: temp_file,
        }
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

#[tokio::test]
async fn test_global_admin_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = kasane::services::auth::generate_jwt("root").unwrap();

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
    let root_token = kasane::services::auth::generate_jwt("root").unwrap();

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
    let root_token = kasane::services::auth::generate_jwt("root").unwrap();

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
    let root_token = kasane::services::auth::generate_jwt("root").unwrap();

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
async fn test_no_privileges() {
    let test_app = PermissionTestApp::new();
    let root_token = kasane::services::auth::generate_jwt("root").unwrap();

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
