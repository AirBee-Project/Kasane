use crate::database::table::common::TestApp;
use crate::database::table::data::common::{assert_first_entry, patch_data, put_data, search_data};
use kasane::models::spatial_id::RawSingleId;

#[tokio::test]
/// upsert (PATCH) により、既存データを保持しつつ重なる部分以外が正しく更新されるかを検証する。
async fn test_table_data_upsert_basic() {
    let test_app = TestApp::new();

    let table_name = "upsert_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int")
        .await;

    let query_a = serde_json::json!([{ "z": 20, "f": 0, "x": 100, "y": 100, "type": "singleId" }]);
    put_data(
        &test_app,
        table_name,
        &serde_json::json!({ "value": 1, "spatial_ids": query_a }),
    )
    .await;

    let query_b = serde_json::json!([{ "z": 19, "f": 0, "x": 50, "y": 50, "type": "singleId" }]);
    patch_data(
        &test_app,
        table_name,
        &serde_json::json!({ "value": 10, "spatial_ids": query_b }),
    )
    .await;

    let res_a = search_data(&test_app, table_name, &query_a).await;
    assert_first_entry(
        &res_a,
        1i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 100,
            y: 100,
            i: None,
            t: None,
        },
    );

    let query_c = serde_json::json!([{ "z": 20, "f": 0, "x": 101, "y": 100, "type": "singleId" }]);
    let res_c = search_data(&test_app, table_name, &query_c).await;
    assert_first_entry(
        &res_c,
        10i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 101,
            y: 100,
            i: None,
            t: None,
        },
    );
}
