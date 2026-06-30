use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use kasane::{AppState, db_init, kasane};
use std::sync::Arc;
use tower::ServiceExt;

pub struct DbTestApp {
    pub app: axum::Router,
    _temp_dir: tempfile::TempDir,
}

impl DbTestApp {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db = db_init::initialize_database(temp_dir.path().to_str().unwrap());

        let app_state = AppState {
            db: db.clone(),
            auth_cache: Arc::new(kasane::auth_cache::AuthCache::new()),
        };
        let token = kasane::services::auth::generate_jwt(&app_state, "root").unwrap();
        let auth_header = axum::http::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap();

        let app = kasane(app_state).layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let auth_header = auth_header.clone();
                async move {
                    req.headers_mut()
                        .insert(axum::http::header::AUTHORIZATION, auth_header);
                    next.run(req).await
                }
            },
        ));
        Self {
            app,
            _temp_dir: temp_dir,
        }
    }
}

#[tokio::test]
/// データベースの作成と一覧取得が正常に行えるかを検証する。
async fn test_create_and_list_database() {
    let test_app = DbTestApp::new();

    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json[0]["name"], "test_db");
}

#[tokio::test]
/// データベースおよび配下のテーブルが正しく削除されるかを検証する。
async fn test_remove_database() {
    let test_app = DbTestApp::new();

    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "test_table", "data_type": "Int", "max_zoom_level": 25}"#,
        ))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_database_remove_cleans_up_privileges() {
    let test_app = DbTestApp::new();

    // 1. Create a user
    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"username": "test_user", "password": "password", "is_global_admin": false}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. Create a database
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 3. Grant privilege to the user
    let req = Request::builder()
        .method("PUT")
        .uri("/users/test_user/privileges/test_db")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"role": "Manage"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Verify privilege exists
    let req = Request::builder()
        .method("GET")
        .uri("/users/test_user/privileges")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);

    // 4. Delete the database
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    // 5. Verify privilege is cleaned up
    let req = Request::builder()
        .method("GET")
        .uri("/users/test_user/privileges")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 0);
}

pub mod table;
