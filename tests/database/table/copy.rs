use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common::TestApp;

#[tokio::test]
/// テーブルのコピーが同一データベース内で正常に行えるかを検証する。
async fn test_table_copy_success_same_db() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    // テーブルを作成
    let create_body = serde_json::json!({
        "name": "src_table",
        "data_type": "Int",
        "max_zoom_level": 25
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();
    let _ = test_app.app.clone().oneshot(req).await.unwrap();

    // テーブルをコピー
    let copy_body = serde_json::json!({
        "copy_table_name": "copied_table"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables/src_table/copy")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&copy_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // コピー先テーブルの情報が取得できるか確認
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/copied_table")
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
