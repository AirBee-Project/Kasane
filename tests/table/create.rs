use crate::common::TestApp;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_create_table_success() {
    let test_app = TestApp::new();

    let create_body = serde_json::json!({
        "name": "new_table",
        "data_type": "Int",
        "max_zoom_level": 25
    });

    let req = Request::builder()
        .method("POST")
        .uri("/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 成功時のステータスコードとLocationヘッダーの確認
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        "/tables/new_table"
    );
}

#[tokio::test]
async fn test_create_table_conflict() {
    let test_app = TestApp::new();

    // 事前にテーブルを作成
    test_app.create_table("existing_table", "Int", 25).await;

    // 同じ名前のテーブルを作成しようとする
    let create_body = serde_json::json!({
        "name": "existing_table",
        "data_type": "Float",
        "max_zoom_level": 20
    });

    let req = Request::builder()
        .method("POST")
        .uri("/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 409 Conflict が返されることを確認
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
