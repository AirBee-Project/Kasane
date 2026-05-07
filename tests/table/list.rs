use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use crate::common::TestApp;

#[tokio::test]
async fn test_table_list_empty() {
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

    // 空の配�Eが返るはぁE
    assert!(body_json.is_array());
    assert_eq!(body_json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_table_list_populated() {
    let test_app = TestApp::new();

    // 2つチE�Eブルを作�E
    test_app.create_table("table_a", "Int", 10).await;
    test_app.create_table("table_b", "Float", 20).await;

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

    // 頁E���E保証されてぁE��ぁE��もしれなぁE�Eで、両方含まれてぁE��か確認すめE
    let names: Vec<&str> = array.iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"table_a"));
    assert!(names.contains(&"table_b"));
}

