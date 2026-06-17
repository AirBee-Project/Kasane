use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::database::table::common::TestApp;

#[tokio::test]
/// テーブルが正常に削除され、再取得できないことを検証する。
async fn test_delete_table_success() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    test_app
        .create_table("test_db", "table_to_delete", "Int", 25)
        .await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db/tables/table_to_delete")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let get_req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/table_to_delete")
        .body(Body::empty())
        .unwrap();

    let get_response = test_app.app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// 存在しないテーブルの削除リクエストが404エラーとなることを検証する。
async fn test_delete_table_not_found() {
    let test_app = TestApp::new();

    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "example_table", "Int", 25)
        .await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db/tables/non_existent_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// データが存在するテーブルを削除した後、同名で再作成できること（キャッシュクリア）を検証する。
async fn test_delete_table_cache_bug() {
    let test_app = TestApp::new();

    let table_name = "bug1_table";

    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let single_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });
    crate::database::table::data::common::put_data(
        &test_app,
        table_name,
        &serde_json::json!({ "value": 1, "query": single_id_query }),
    )
    .await;

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/databases/test_db/tables/{}", table_name))
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "name": table_name,
                "data_type": "Int",
                "max_zoom_level": 25
            }))
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Table should be recreatable after deletion"
    );
}
