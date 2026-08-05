use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::database::table::common::TestApp;

#[tokio::test]
/// 初期状態で空のテーブル一覧が取得できることを検証する。
async fn test_table_list_empty() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.is_array());
    assert_eq!(body_json.as_array().unwrap().len(), 0);
}

#[tokio::test]
/// 2つのテーブルを追加し、一覧に両方が含まれていることを検証する。
async fn test_table_list_two() {
    let test_app = TestApp::new();

    test_app.create_database("test_db").await;
    test_app.create_table("test_db", "table_a", "Int").await;
    test_app
        .create_table("test_db", "table_b", "Float")
        .await;

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.is_array());
    let array = body_json.as_array().unwrap();
    assert_eq!(array.len(), 2);

    let names: Vec<&str> = array.iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"table_a"));
    assert!(names.contains(&"table_b"));
}

#[tokio::test]
/// 3つのテーブルを追加し、一覧にすべてが含まれていることを検証する。
async fn test_table_list_three() {
    let test_app = TestApp::new();

    test_app.create_database("test_db").await;
    test_app.create_table("test_db", "table_a", "Int").await;
    test_app
        .create_table("test_db", "table_b", "Float")
        .await;
    test_app
        .create_table("test_db", "table_c", "Text")
        .await;

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.is_array());
    let array = body_json.as_array().unwrap();
    assert_eq!(array.len(), 3);

    let names: Vec<&str> = array.iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"table_a"));
    assert!(names.contains(&"table_b"));
    assert!(names.contains(&"table_c"));
}

#[tokio::test]
/// db_nameが空文字列の場合に500エラー(MDB_BAD_VALSIZE等)にならず、404エラーになることを検証する。
async fn test_table_list_empty_db_name() {
    let test_app = TestApp::new();

    let req = Request::builder()
        .method("GET")
        .uri("/databases//tables")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // Axumのルーティングで弾かれるか、RepositoryでDatabaseNotFoundが返って404になるはず
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
