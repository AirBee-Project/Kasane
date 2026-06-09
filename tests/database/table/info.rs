use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::database::table::common::TestApp;

#[tokio::test]
/// 正しく情報を取得できているかを検証する
async fn test_table_info_success() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    // 事前にレイヤーを作成
    test_app
        .create_table("test_db", "info_target_table", "Float", 15)
        .await;

    // レイヤー情報の取得リクエスト
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/info_target_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["name"], "info_target_table");
    assert_eq!(body_json["data_type"], "Float");
    assert_eq!(body_json["max_zoom_level"], 15);
}

#[tokio::test]
/// 存在しないレイヤーの情報が取得できないことを確認する
async fn test_table_info_not_found() {
    let test_app = TestApp::new();

    // ダミーのレイヤーを作成
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "example_table", "Int", 25)
        .await;

    // 存在しないレイヤーの情報を取得しようとする
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/non_existent_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 404 Not Found が返されることを確認
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
