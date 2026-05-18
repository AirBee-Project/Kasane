use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::layer::common::TestApp;

#[tokio::test]
async fn test_auth_no_keys() {
    let test_app = TestApp::with_keys(None, None);

    // Read should succeed
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/layers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Write should succeed
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/layers")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"name": "test_layer", "data_type": "Int", "max_zoom_level": 25}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_auth_readonly_key_set() {
    let test_app = TestApp::with_keys(Some("read_secret".to_string()), None);

    // Read without key should fail
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/layers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Read with readonly key should succeed
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/layers")
                .header("Authorization", "Bearer read_secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Write should succeed even without key (since no write key is set)
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/layers")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"name": "test_layer", "data_type": "Int", "max_zoom_level": 25}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_auth_write_key_set() {
    let test_app = TestApp::with_keys(None, Some("write_secret".to_string()));

    // Read without key should succeed (since no readonly key is set)
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/layers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Write without key should fail
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/layers")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"name": "test_layer", "data_type": "Int", "max_zoom_level": 25}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Write with write key should succeed
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/layers")
                .header("x-api-key", "write_secret")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"name": "test_layer", "data_type": "Int", "max_zoom_level": 25}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_auth_both_keys_set() {
    let test_app = TestApp::with_keys(
        Some("read_secret".to_string()),
        Some("write_secret".to_string()),
    );

    // Read with write key should succeed
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/layers")
                .header("Authorization", "Bearer write_secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Write with read key should fail
    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/layers")
                .header("x-api-key", "read_secret")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"name": "test_layer", "data_type": "Int", "max_zoom_level": 25}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
