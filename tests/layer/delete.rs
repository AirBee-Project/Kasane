use crate::common::TestApp;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_delete_layer_success() {
    let test_app = TestApp::new();

    // 事前にレイヤーを作成
    test_app.create_layer("layer_to_delete", "Int", 25).await;

    // レイヤーの削除リクエスト
    let req = Request::builder()
        .method("DELETE")
        .uri("/layers/layer_to_delete")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 成功時は 204 No Content が返される
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 削除後に情報取得リクエストを投げると 404 Not Found になることを確認
    let get_req = Request::builder()
        .method("GET")
        .uri("/layers/layer_to_delete")
        .body(Body::empty())
        .unwrap();

    let get_response = test_app.app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_layer_not_found() {
    let test_app = TestApp::new();

    // 存在しないレイヤーを削除しようとする
    let req = Request::builder()
        .method("DELETE")
        .uri("/layers/non_existent_layer")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 404 Not Found が返されることを確認
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
