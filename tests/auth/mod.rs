use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::layer::common::TestApp;

/// 【テスト】認証キーが一切設定されていない（パブリック）場合の挙動
///
/// 読み取り・書き込みの両方の操作が、キーなしで正常に成功することを確認します。
#[tokio::test]
async fn test_auth_no_keys() {
    let test_app = TestApp::with_keys(None, None);

    // 読み取り（READ）操作がキーなしで成功すること
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

    // 書き込み（WRITE）操作がキーなしで成功すること
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

/// 【テスト】読み取りキー（READ_KEY）のみが設定されている場合の挙動
///
/// - 読み取り（READ）操作にはキーが必要になり、正しいキーがなければ拒否されること。
/// - 書き込み（WRITE）操作は、キーが未設定のため認証なしで成功すること。
#[tokio::test]
async fn test_auth_read_key_set() {
    let test_app = TestApp::with_keys(Some("read_secret".to_string()), None);

    // 読み取り：キーがない場合は UNAUTHORIZED (401) エラーになること
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

    // 読み取り：正しい READ_KEY を提示した場合は成功すること
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

    // 書き込み：書き込みキーが未設定のため、キーなしでも書き込みが成功すること
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

/// 【テスト】書き込みキー（WRITE_KEY）のみが設定されている場合の挙動
///
/// - 読み取り（READ）操作は、読み取りキーが未設定のためキーなしで成功すること。
/// - 書き込み（WRITE）操作には正しい WRITE_KEY が必須であること。
#[tokio::test]
async fn test_auth_write_key_set() {
    let test_app = TestApp::with_keys(None, Some("write_secret".to_string()));

    // 読み取り：読み取りキーが未設定のため、キーなしで成功すること
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

    // 書き込み：キーがない場合は UNAUTHORIZED (401) エラーになること
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

    // 書き込み：正しい WRITE_KEY を提示した場合は成功すること
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

/// 【テスト】読み取り・書き込み両方のキーが設定されている場合の相互排他・昇格挙動
///
/// - WRITE_KEY を持っていれば、読み取り（READ）操作もパスできること（権限の昇格）。
/// - READ_KEY では、書き込み（WRITE）操作はパスできず拒否されること。
#[tokio::test]
async fn test_auth_both_keys_set() {
    let test_app = TestApp::with_keys(
        Some("read_secret".to_string()),
        Some("write_secret".to_string()),
    );

    // 読み取り：より強い権限の WRITE_KEY を提示した場合でも成功すること
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

    // 書き込み：READ_KEY を提示しても、書き込み操作は拒否 (401) されること
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
