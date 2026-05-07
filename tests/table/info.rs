use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use crate::common::TestApp;

#[tokio::test]
async fn test_table_info_success() {
    let test_app = TestApp::new();

    // 事前にチE�Eブルを作�E
    test_app.create_table("info_target_table", "Float", 15).await;

    // チE�Eブル惁E��の取得リクエスチE
    let req = Request::builder()
        .method("GET")
        .uri("/layers/info_target_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 成功時�E 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // ボディの検証
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["name"], "info_target_table");
    assert_eq!(body_json["data_type"], "Float");
    assert_eq!(body_json["max_zoom_level"], 15);
}

#[tokio::test]
async fn test_table_info_not_found() {
    let test_app = TestApp::new();

    // 存在しなぁE��ーブルの惁E��を取得しようとする
    let req = Request::builder()
        .method("GET")
        .uri("/layers/non_existent_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 404 Not Found が返されることを確誁E
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

