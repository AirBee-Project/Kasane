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
        .uri("/layers")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 成功時�EスチE�EタスコードとLocationヘッダーの確誁E
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        "/layers/new_table"
    );
}

#[tokio::test]
async fn test_create_table_conflict() {
    let test_app = TestApp::new();

    // 事前にチE�Eブルを作�E
    test_app.create_table("existing_table", "Int", 25).await;

    // 同じ名前のチE�Eブルを作�EしよぁE��する
    let create_body = serde_json::json!({
        "name": "existing_table",
        "data_type": "Float",
        "max_zoom_level": 20
    });

    let req = Request::builder()
        .method("POST")
        .uri("/layers")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 409 Conflict が返されることを確誁E
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

