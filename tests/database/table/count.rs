use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::common::TestApp;
use crate::common::data::put_data;

async fn get_table_count(test_app: &TestApp, table_name: &str) -> u64 {
    let req = Request::builder()
        .method("GET")
        .uri(format!("/databases/test_db/tables/{}", table_name))
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    body_json["count"].as_u64().unwrap()
}

#[tokio::test]
/// データの挿入・更新・削除に伴い、テーブルの count が正しく増減するかを検証する。
async fn test_table_count_dynamic() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "count_test_table", "Int", 25)
        .await;

    // 初期状態の count は 0 であること
    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 0, "Initial count should be 0");

    // 1件目のデータを挿入
    let single_id_query_1 =
        serde_json::json!([{ "z": 20, "f": 0, "x": 100, "y": 100, "type": "singleId" }]);
    put_data(
        &test_app,
        "count_test_table",
        &serde_json::json!({ "value": 1, "spatial_ids": single_id_query_1 }),
    )
    .await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 1, "Count should be 1 after one insert");

    // 2件目のデータを挿入
    let single_id_query_2 =
        serde_json::json!([{ "z": 20, "f": 0, "x": 200, "y": 200, "type": "singleId" }]);
    put_data(
        &test_app,
        "count_test_table",
        &serde_json::json!({ "value": 2, "spatial_ids": single_id_query_2 }),
    )
    .await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 2, "Count should be 2 after second insert");

    // 既存のデータを上書き（count は 2 のまま変わらないこと）
    put_data(
        &test_app,
        "count_test_table",
        &serde_json::json!({ "value": 3, "spatial_ids": single_id_query_2 }),
    )
    .await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 2, "Count should remain 2 after overwrite");

    // range を使って範囲で挿入（例: z=21 の FlexId を 4 つ追加）
    let range_id_query = serde_json::json!([{ "z": 21, "f": [0,0], "x": [1000, 1001], "y": [1000, 1001], "type": "rangeId" }]);
    put_data(
        &test_app,
        "count_test_table",
        &serde_json::json!({ "value": 4, "spatial_ids": range_id_query }),
    )
    .await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(
        count, 3,
        "Count should be 3 after adding 4 flex_ids via range (merged into 1 parent block)"
    );

    // 1件目のデータを削除
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db/tables/count_test_table/data")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "spatial_ids": single_id_query_1 }))
                .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 2, "Count should be 2 after deleting 1 item");
}
