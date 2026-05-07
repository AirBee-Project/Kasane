use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use crate::common::TestApp;

#[tokio::test]
async fn test_delete_table_success() {
    let test_app = TestApp::new();

    // 事前にチE�Eブルを作�E
    test_app.create_table("table_to_delete", "Int", 25).await;

    // チE�Eブルの削除リクエスチE
    let req = Request::builder()
        .method("DELETE")
        .uri("/layers/table_to_delete")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 成功時�E 204 No Content が返される
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 削除後に惁E��取得リクエストを投げると 404 Not Found になることを確誁E
    let get_req = Request::builder()
        .method("GET")
        .uri("/layers/table_to_delete")
        .body(Body::empty())
        .unwrap();

    let get_response = test_app.app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_table_not_found() {
    let test_app = TestApp::new();

    // 存在しなぁE��ーブルを削除しよぁE��する
    let req = Request::builder()
        .method("DELETE")
        .uri("/layers/non_existent_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 404 Not Found が返されることを確誁E
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

