use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use kasane::{AppState, db_init, kasane};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tower::ServiceExt;

pub struct DbTestApp {
    pub app: axum::Router,
    _temp_file: NamedTempFile,
}

impl DbTestApp {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = db_init::initialize_database(temp_file.path().to_str().unwrap());
        let app_state = AppState { redb: Arc::new(db) };
        let app = kasane(app_state);
        Self {
            app,
            _temp_file: temp_file,
        }
    }
}

#[tokio::test]
async fn test_create_and_list_database() {
    let test_app = DbTestApp::new();

    // List empty
    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Create DB
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // List again
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
async fn test_remove_database() {
    let test_app = DbTestApp::new();

    // Create DB
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    // Create Table inside DB
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "test_table", "data_type": "Int", "max_zoom_level": 25}"#,
        ))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    // Remove DB
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Verify DB is gone
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

pub mod table;
