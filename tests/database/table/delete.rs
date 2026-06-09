use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::database::table::common::TestApp;

#[tokio::test]
/// tableが正しく削除できることを確認する
async fn test_delete_table_success() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    // 事前にレイヤーを作成
    test_app
        .create_table("test_db", "table_to_delete", "Int", 25)
        .await;

    // レイヤーの削除リクエスト
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db/tables/table_to_delete")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 削除できていることを確認
    let get_req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/table_to_delete")
        .body(Body::empty())
        .unwrap();

    let get_response = test_app.app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// 存在しないTableを削除できていることを検証する
async fn test_delete_table_not_found() {
    let test_app = TestApp::new();

    // ダミーのレイヤーを作成
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "example_table", "Int", 25)
        .await;

    // 存在しないレイヤーを削除しようとする
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db/tables/non_existent_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    // 404 Not Found が返されることを確認
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// データが存在するTableを削除した後、同じ名前で再作成できることを確認する (Bug 1 の検証)
async fn test_delete_table_cache_bug() {
    let test_app = TestApp::new();

    let table_name = "bug1_table";

    // 1. レイヤーを作成
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    // 2. データを挿入 (これにより削除時のデータ削除ループが実行される)
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

    // 3. レイヤーを削除
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/databases/test_db/tables/{}", table_name))
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 4. 同じ名前で再度作成を試みる
    // バグがある場合、キャッシュに残っているため「既に存在する」と誤判定される可能性がある
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

    // 期待値は 201 Created だが、バグがあると 409 Conflict (TableAlreadyExists) が返る
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Table should be recreatable after deletion"
    );
}
