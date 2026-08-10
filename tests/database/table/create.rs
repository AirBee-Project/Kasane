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
        "data_type": "Int",
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

#[tokio::test]
/// ENUM型のテーブル作成時に、選択肢の文字列長さが制限(最大255文字、空文字禁止)に従っているか検証する。
async fn test_create_table_enum_choice_length_limits() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    // 1. 256文字の選択肢（エラーになるべき）
    let long_choice = "a".repeat(256);
    let create_body_too_long = serde_json::json!({
        "name": "too_long_enum",
        "data_type": "Enum",
        "max_zoom_level": 25,
        "constraints": {
            "type": "Enum",
            "choices": [long_choice]
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&create_body_too_long).unwrap(),
        ))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 2. 空文字の選択肢（エラーになるべき）
    let create_body_empty = serde_json::json!({
        "name": "empty_enum",
        "data_type": "Enum",
        "max_zoom_level": 25,
        "constraints": {
            "type": "Enum",
            "choices": [""]
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&create_body_empty).unwrap(),
        ))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 3. 255文字の選択肢（成功するべき）
    let border_choice = "a".repeat(255);
    let create_body_ok = serde_json::json!({
        "name": "ok_enum",
        "data_type": "Enum",
        "max_zoom_level": 25,
        "constraints": {
            "type": "Enum",
            "choices": [border_choice]
        }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body_ok).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
/// テーブルのdescription付与が正常に行えるかを検証する。
async fn test_create_table_description() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "desc_table",
        "data_type": "Int",
        "max_zoom_level": 25,
        "description": "This is a test table."
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/desc_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["description"], "This is a test table.");
}

#[tokio::test]
/// テーブルのdescriptionが4096文字を超える場合にエラーになるかを検証する。
async fn test_create_table_description_too_long() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let long_desc = "a".repeat(kasane::models::database::MAX_DESCRIPTION_LENGTH + 1);

    let create_body = serde_json::json!({
        "name": "desc_table_too_long",
        "data_type": "Int",
        "max_zoom_level": 25,
        "description": long_desc
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
