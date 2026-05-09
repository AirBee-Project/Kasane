use crate::common::TestApp;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
/// 初期状態を検証する
async fn test_layer_list_empty() {
    let test_app = TestApp::new();

    let req = Request::builder()
        .method("GET")
        .uri("/layers")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // 空の配列が返るはず
    assert!(body_json.is_array());
    assert_eq!(body_json.as_array().unwrap().len(), 0);
}

#[tokio::test]
/// 2つのLayerを追加する
async fn test_layer_list_two() {
    let test_app = TestApp::new();

    // 2つレイヤーを作成
    test_app.create_layer("layer_a", "Int", 10).await;
    test_app.create_layer("layer_b", "Float", 20).await;

    let req = Request::builder()
        .method("GET")
        .uri("/layers")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.is_array());
    let array = body_json.as_array().unwrap();
    assert_eq!(array.len(), 2);

    // 順序は保証されていないかもしれないので、両方含まれているか確認する
    let names: Vec<&str> = array.iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"layer_a"));
    assert!(names.contains(&"layer_b"));
}

#[tokio::test]
/// 2つのLayerを追加する
async fn test_layer_list_three() {
    let test_app = TestApp::new();

    // 2つレイヤーを作成
    test_app.create_layer("layer_a", "Int", 10).await;
    test_app.create_layer("layer_b", "Float", 20).await;
    test_app.create_layer("layer_c", "Text", 25).await;

    let req = Request::builder()
        .method("GET")
        .uri("/layers")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.is_array());
    let array = body_json.as_array().unwrap();
    assert_eq!(array.len(), 3);

    // 順序は保証されていないかもしれないので、両方含まれているか確認する
    let names: Vec<&str> = array.iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"layer_a"));
    assert!(names.contains(&"layer_b"));
    assert!(names.contains(&"layer_c"));
}
