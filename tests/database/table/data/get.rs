use crate::database::table::common::TestApp;
use crate::database::table::data::common::{put_data, search_data, to_result_map};
use kasane::models::spatial_id::RawSingleId;

#[tokio::test]
/// 複数の空間IDを一度に指定してデータを検索・取得できることを検証する。
async fn test_table_data_get_multiple() {
    let test_app = TestApp::new();

    let table_name = "get_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let id1 = RawSingleId {
        z: 20,
        f: 0,
        x: 10,
        y: 10,
    };
    let id2 = RawSingleId {
        z: 20,
        f: 0,
        x: 20,
        y: 20,
    };

    put_data(
        &test_app,
        table_name,
        &serde_json::json!({
            "value": 100,
            "spatial_ids": [{ "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" }]
        }),
    )
    .await;

    put_data(
        &test_app,
        table_name,
        &serde_json::json!({
            "value": 200,
            "spatial_ids": [{ "z": 20, "f": 0, "x": 20, "y": 20, "type": "singleId" }]
        }),
    )
    .await;

    let query = serde_json::json!([
        { "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" },
        { "z": 20, "f": 0, "x": 20, "y": 20, "type": "singleId" }
    ]);

    let result_json = search_data(&test_app, table_name, &query).await;
    let result_map = to_result_map::<i64>(&result_json);

    assert_eq!(result_map.len(), 2);
    assert_eq!(result_map[&id1], 100);
    assert_eq!(result_map[&id2], 200);
}
