use crate::common::TestApp;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
/// layerを作成して、正しく作成できていることを確認する
async fn test_create_layer_success() {
    let test_app = TestApp::new();

    // 作成する
    let create_body = serde_json::json!({
        "name": "new_layer",
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

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        "/layers/new_layer"
    );

    // 作成できているかを検証する
    let req = Request::builder()
        .method("GET")
        .uri("/layers/new_layer")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // JSON 内容を検証
    assert_eq!(json["name"], "new_layer");
    assert_eq!(json["data_type"], "Int");
    assert_eq!(json["max_zoom_level"], 25);
}

#[tokio::test]
/// 同じ名前のlayerを作成して、作成ができないことを確認する
/// また、もともとあったLayerが消えていないことを確認する。
async fn test_create_layer_conflict() {
    let test_app = TestApp::new();

    // 事前にレイヤーを作成
    test_app.create_layer("existing_layer", "Int", 25).await;

    // 同じ名前のレイヤーを作成しようとする
    let create_body = serde_json::json!({
        "name": "existing_layer",
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

    assert_eq!(response.status(), StatusCode::CONFLICT);

    // もともとあったLayerが今も正しく存在するかを検証する
    let req = Request::builder()
        .method("GET")
        .uri("/layers/existing_layer")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // JSON 内容を検証
    assert_eq!(json["name"], "existing_layer");
    assert_eq!(json["data_type"], "Int");
    assert_eq!(json["max_zoom_level"], 25);
}
