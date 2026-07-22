//! `POST /query` の統合テスト。
//!
//! クエリはシャードされた FlexTree を Kasane-Logic の入力源として読み、
//! 対象領域だけを遅延評価する。ここではその end-to-end の挙動を検証する。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

mod routing;
mod values;

use crate::database::table::common::TestApp;
use crate::database::table::data::common::put_data;

/// `POST /query` を実行し、`(status, body)` を返す。
async fn post_query(
    test_app: &TestApp,
    body: &serde_json::Value,
    query_string: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/query{}", query_string))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn single_id(x: i64) -> serde_json::Value {
    serde_json::json!({ "z": 20, "f": 0, "x": x, "y": 500000, "type": "singleId" })
}

/// 値辞書 + データ群のレスポンスを、値の合計と件数へ畳み込む。
fn total_ids(result: &serde_json::Value) -> usize {
    result["data"]
        .as_array()
        .map(|groups| {
            groups
                .iter()
                .map(|g| g["spatialIds"].as_array().map(|a| a.len()).unwrap_or(0))
                .sum()
        })
        .unwrap_or(0)
}

/// 出力に現れた値の集合を返す。
fn values(result: &serde_json::Value) -> Vec<i64> {
    let dict = result["dictionary"].as_array().cloned().unwrap_or_default();
    let mut out: Vec<i64> = result["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|g| {
            let idx = g["valueRef"].as_u64()? as usize;
            dict.get(idx)?.as_i64()
        })
        .collect();
    out.sort_unstable();
    out
}

/// 演算子を挟まない素通しクエリが、格納した値をそのまま返す。
#[tokio::test]
async fn query_source_only_returns_stored_values() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 42, "spatial_ids": [single_id(600000)] }),
    )
    .await;

    let (status, result) = post_query(
        &test_app,
        &serde_json::json!({
            "spatial_ids": [single_id(600000)],
            "query": { "type": "source", "database": "test_db", "table": "test_table" }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(values(&result), vec![42]);
}

/// shiftX で値が隣のセルへ移動する（元の位置には現れない）。
#[tokio::test]
async fn query_shift_x_moves_values() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 7, "spatial_ids": [single_id(610000)] }),
    )
    .await;

    let query = serde_json::json!({
        "type": "shiftX",
        "input": { "type": "source", "database": "test_db", "table": "test_table" },
        "z": 20,
        "index": 3
    });

    // 移動先には現れる
    let (status, moved) = post_query(
        &test_app,
        &serde_json::json!({ "spatial_ids": [single_id(610003)], "query": query }),
        "?format=singleId",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {moved}");
    assert_eq!(values(&moved), vec![7]);

    // 元の位置には残らない
    let (status, origin) = post_query(
        &test_app,
        &serde_json::json!({ "spatial_ids": [single_id(610000)], "query": query }),
        "?format=singleId",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(total_ids(&origin), 0);
}

/// merge で2つのテーブル（別データベース）を1つのクエリで合成できる。
#[tokio::test]
async fn query_merge_across_two_databases() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;
    test_app.create_database("other_db").await;
    test_app
        .create_table("other_db", "other_table", "Int", 25)
        .await;

    // 同じ空間IDへ、別々のデータベースのテーブルから 10 と 5 を置く。
    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 10, "spatial_ids": [single_id(620000)] }),
    )
    .await;

    let req = Request::builder()
        .method("PUT")
        .uri("/databases/other_db/tables/other_table/data")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({ "value": 5, "spatial_ids": [single_id(620000)] }).to_string(),
        ))
        .unwrap();
    assert_eq!(
        test_app.app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );

    let (status, result) = post_query(
        &test_app,
        &serde_json::json!({
            "spatial_ids": [single_id(620000)],
            "query": {
                "type": "merge",
                "left":  { "type": "source", "database": "test_db",  "table": "test_table" },
                "right": { "type": "source", "database": "other_db", "table": "other_table" },
                "default": 0,
                "policy": "sum"
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![15], "10 + 5 = 15 が返るはず");
}

/// data_type が異なるテーブルを混在させると 400 で拒否される。
#[tokio::test]
async fn query_rejects_mixed_data_types() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "int_table", "Int", 25)
        .await;
    test_app
        .create_table("test_db", "text_table", "Text", 25)
        .await;

    let (status, _) = post_query(
        &test_app,
        &serde_json::json!({
            "spatial_ids": [single_id(630000)],
            "query": {
                "type": "merge",
                "left":  { "type": "source", "database": "test_db", "table": "int_table" },
                "right": { "type": "source", "database": "test_db", "table": "text_table" },
                "default": 0,
                "policy": "sum"
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 存在しないテーブルを参照すると 404。
#[tokio::test]
async fn query_missing_table_returns_404() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let (status, _) = post_query(
        &test_app,
        &serde_json::json!({
            "spatial_ids": [single_id(640000)],
            "query": { "type": "source", "database": "test_db", "table": "nope" }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
