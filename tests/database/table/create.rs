use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use crate::database::table::common::TestApp;

#[tokio::test]
/// tableを作成して、正しく作成できていることを確認する
async fn test_create_table_success() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    // 作成する
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

    // 作成できているかを検証する
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/new_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // JSON 内容を検証
    assert_eq!(json["name"], "new_table");
    assert_eq!(json["data_type"], "Int");
    assert_eq!(json["max_zoom_level"], 25);
}

#[tokio::test]
/// 同じ名前のtableを作成して、作成ができないことを確認する
/// また、もともとあったTableが消えていないことを確認する。
async fn test_create_table_conflict() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    // 事前にレイヤーを作成
    test_app
        .create_table("test_db", "existing_table", "Int", 25)
        .await;

    // 同じ名前のレイヤーを作成しようとする
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

    // もともとあったTableが今も正しく存在するかを検証する
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/existing_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // JSON 内容を検証
    assert_eq!(json["name"], "existing_table");
    assert_eq!(json["data_type"], "Int");
    assert_eq!(json["max_zoom_level"], 25);
}
