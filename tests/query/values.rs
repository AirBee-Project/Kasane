//! 値に注目したクエリ（値フィルタ・型変換）と、全データ型への対応の検証。

use axum::http::StatusCode;

use super::{post_query, single_id, total_ids, values};
use crate::database::table::common::TestApp;
use crate::database::table::data::common::put_data;

/// `test_db` に指定型のテーブルを作り、`(x, 値)` を書き込む。
async fn seed(
    test_app: &TestApp,
    table: &str,
    data_type: &str,
    cells: &[(i64, serde_json::Value)],
) {
    test_app.create_table("test_db", table, data_type, 25).await;
    for (x, v) in cells {
        put_data(
            test_app,
            table,
            &serde_json::json!({ "value": v, "spatial_ids": [single_id(*x)] }),
        )
        .await;
    }
}

fn source(table: &str) -> serde_json::Value {
    serde_json::json!({ "type": "source", "database": "test_db", "table": table })
}

fn ids(base: i64, count: i64) -> Vec<serde_json::Value> {
    (0..count).map(|i| single_id(base + i)).collect()
}

/// ある値のみを残す。
#[tokio::test]
async fn filter_equals_keeps_only_that_value() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_eq",
        "Int",
        &[
            (700000, serde_json::json!(1)),
            (700001, serde_json::json!(5)),
            (700002, serde_json::json!(10)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(700000, 3),
            "query": {
                "type": "filterValues", "mode": "equals", "value": 5,
                "input": source("t_eq")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![5]);
    assert_eq!(total_ids(&result), 1);
}

/// ある範囲の値を残す（閉区間）。
#[tokio::test]
async fn filter_in_range_is_inclusive() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_in",
        "Int",
        &[
            (710000, serde_json::json!(1)),
            (710001, serde_json::json!(5)),
            (710002, serde_json::json!(10)),
            (710003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(710000, 4),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 5, "max": 10,
                "input": source("t_in")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![5, 10], "境界を含むこと");
}

/// ある範囲の値**以外**を残す。
#[tokio::test]
async fn filter_not_in_range_keeps_the_outside() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_out",
        "Int",
        &[
            (720000, serde_json::json!(1)),
            (720001, serde_json::json!(5)),
            (720002, serde_json::json!(10)),
            (720003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(720000, 4),
            "query": {
                "type": "filterValues", "mode": "notInRange", "min": 5, "max": 10,
                "input": source("t_out")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![1, 20]);
}

/// 変換表で Text テーブルを Int のクエリとして扱う（型が異なる値への置き換え）。
#[tokio::test]
async fn convert_maps_text_source_into_int() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_text",
        "Text",
        &[
            (730000, serde_json::json!("low")),
            (730001, serde_json::json!("high")),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(730000, 2),
            "query": {
                "type": "source", "database": "test_db", "table": "t_text",
                "convert": {
                    "entries": [{ "from": "low", "to": 1 }, { "from": "high", "to": 9 }]
                }
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![1, 9]);
}

/// 変換表に無い値は、`default` 未指定なら結果から除外される。
#[tokio::test]
async fn convert_drops_unmapped_values_without_default() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_text2",
        "Text",
        &[
            (740000, serde_json::json!("low")),
            (740001, serde_json::json!("unknown")),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(740000, 2),
            "query": {
                "type": "source", "database": "test_db", "table": "t_text2",
                "convert": { "entries": [{ "from": "low", "to": 1 }] }
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![1]);
    assert_eq!(total_ids(&result), 1, "未掲載値のセルは落ちる");
}

/// `default` を指定すると、未掲載値もその値で埋まる。
#[tokio::test]
async fn convert_fills_unmapped_values_with_default() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_text3",
        "Text",
        &[
            (745000, serde_json::json!("low")),
            (745001, serde_json::json!("unknown")),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(745000, 2),
            "query": {
                "type": "source", "database": "test_db", "table": "t_text3",
                "convert": { "entries": [{ "from": "low", "to": 1 }], "default": 0 }
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![0, 1]);
    assert_eq!(total_ids(&result), 2);
}

/// 型の異なる2テーブル（Text と Int）を変換表で揃えて合成できる。
#[tokio::test]
async fn convert_enables_merging_different_source_types() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_a", "Text", &[(750000, serde_json::json!("high"))]).await;
    seed(&app, "t_b", "Int", &[(750000, serde_json::json!(4))]).await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(750000, 1),
            "query": {
                "type": "merge", "default": 0, "policy": "sum",
                "left": {
                    "type": "source", "database": "test_db", "table": "t_a",
                    "convert": { "entries": [{ "from": "high", "to": 6 }] }
                },
                "right": source("t_b")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![10], "6 + 4 = 10");
}

/// Text テーブルでも値フィルタは使える（比較に必要なのは順序だけ）。
#[tokio::test]
async fn filter_works_on_text_values() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_words",
        "Text",
        &[
            (760000, serde_json::json!("apple")),
            (760001, serde_json::json!("banana")),
            (760002, serde_json::json!("cherry")),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(760000, 3),
            "query": {
                "type": "filterValues", "mode": "equals", "value": "banana",
                "input": source("t_words")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(result["dictionary"][0], serde_json::json!("banana"));
}

/// Boolean テーブルにもクエリを適用できる。
#[tokio::test]
async fn supports_boolean_tables() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_flag",
        "Boolean",
        &[
            (770000, serde_json::json!(true)),
            (770001, serde_json::json!(false)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(770000, 2),
            "query": {
                "type": "filterValues", "mode": "equals", "value": true,
                "input": source("t_flag")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(result["dictionary"][0], serde_json::json!(true));
}

/// Float テーブル（`Ord` ラッパー経由）にもクエリを適用できる。
#[tokio::test]
async fn supports_float_tables() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_float",
        "Float",
        &[
            (775000, serde_json::json!(1.5)),
            (775001, serde_json::json!(9.5)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(775000, 2),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 5.0,
                "input": source("t_float")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(result["dictionary"][0], serde_json::json!(9.5));
}

/// BigInt テーブルにもクエリを適用できる。
#[tokio::test]
async fn supports_bigint_tables() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_big",
        "BigInt",
        &[
            (778000, serde_json::json!(1_000_000_000_000i64)),
            (778001, serde_json::json!(1i64)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(778000, 2),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 1_000_000i64,
                "input": source("t_big")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(
        result["dictionary"][0],
        serde_json::json!(1_000_000_000_000i64)
    );
}

/// 算術を要する演算子は非数値型では 400 になる。
#[tokio::test]
async fn rejects_arithmetic_operator_on_text() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_txt3", "Text", &[]).await;

    let (status, _) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(780000, 1),
            "query": {
                "type": "falloffLinearX", "z": 20, "radius": 2, "policy": "max",
                "input": source("t_txt3")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 数値専用の `MergePolicy` を非数値型に使うと 400 になる。
#[tokio::test]
async fn rejects_numeric_policy_on_text() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_txt4", "Text", &[]).await;

    let (status, _) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(785000, 1),
            "query": {
                "type": "zoomOut", "z": 19, "policy": "sum",
                "input": source("t_txt4")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 演算子を挟まないクエリで、対象領域の全セルが返ること。
///
/// 対象領域の求め方を誤ると一部のセルだけが返る退行が起きるため、複数セルで固定する。
#[tokio::test]
async fn source_only_returns_all_cells() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_multi",
        "Int",
        &[
            (790000, serde_json::json!(1)),
            (790001, serde_json::json!(5)),
            (790002, serde_json::json!(10)),
            (790003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(790000, 4),
            "query": source("t_multi")
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![1, 5, 10, 20], "body: {result}");
}
