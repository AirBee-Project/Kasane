use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use crate::database::table::common::TestApp;

#[tokio::test]
/// テーブルの正常な作成と取得を検証する。
async fn test_create_table_success() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "new_table",
        "data_type": "Int",
        "max_zoom_level": 25
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        "/databases/test_db/tables/new_table"
    );

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/new_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["name"], "new_table");
    assert_eq!(json["data_type"], "Int");
    assert_eq!(json["max_zoom_level"], 25);
}

#[tokio::test]
/// 同名テーブルの作成が競合エラーとなり、既存のテーブルが保持されることを検証する。
async fn test_create_table_conflict() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    test_app
        .create_table("test_db", "existing_table", "Int", 25)
        .await;

    let create_body = serde_json::json!({
        "name": "existing_table",
        "data_type": "Float",
        "max_zoom_level": 20
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/existing_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["name"], "existing_table");
    assert_eq!(json["data_type"], "Int");
    assert_eq!(json["max_zoom_level"], 25);
}

#[tokio::test]
/// max_zoom_level がシステム上限(30)を超える場合は 400 で拒否され、テーブルは作成されない。
async fn test_create_table_max_zoom_level_too_large() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "too_deep",
        "data_type": "Int",
        "max_zoom_level": 31
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 検証で弾かれているので、テーブルは作成されていない。
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/too_deep")
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// max_zoom_level の境界値 30（システム上限）は許可される。
async fn test_create_table_max_zoom_level_boundary_ok() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "boundary_table",
        "data_type": "Int",
        "max_zoom_level": 30
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}
