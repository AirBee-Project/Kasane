use crate::layer::common::TestApp;
use crate::layer::data::common::{put_data, search_data, to_result_map};
use kasane::models::spatial_id::RawSingleId;

#[tokio::test]
/// 複数の空間IDを一度に指定して検索できることを検証する
async fn test_layer_data_get_multiple() {
    let test_app = TestApp::new();
    let layer_name = "get_layer";
    test_app.create_layer(layer_name, "Int", 25).await;

    // 2つの異なる場所にデータを挿入
    let id1 = RawSingleId { z: 20, f: 0, x: 10, y: 10 };
    let id2 = RawSingleId { z: 20, f: 0, x: 20, y: 20 };

    put_data(
        &test_app,
        layer_name,
        &serde_json::json!({
            "value": 100,
            "query": { "ids": [{ "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" }], "type": "spatialIds" }
        }),
    )
    .await;

    put_data(
        &test_app,
        layer_name,
        &serde_json::json!({
            "value": 200,
            "query": { "ids": [{ "z": 20, "f": 0, "x": 20, "y": 20, "type": "singleId" }], "type": "spatialIds" }
        }),
    )
    .await;

    // 2つ同時に検索
    let query = serde_json::json!({
        "ids": [
            { "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" },
            { "z": 20, "f": 0, "x": 20, "y": 20, "type": "singleId" }
        ],
        "type": "spatialIds"
    });

    let result_json = search_data(&test_app, layer_name, &query).await;
    let result_map = to_result_map::<i64>(&result_json);

    assert_eq!(result_map.len(), 2);
    assert_eq!(result_map[&id1], 100);
    assert_eq!(result_map[&id2], 200);
}
