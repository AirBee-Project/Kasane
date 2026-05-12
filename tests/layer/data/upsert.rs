use crate::layer::common::TestApp;
use crate::layer::data::common::{assert_first_entry, patch_data, put_data, search_data};
use kasane::models::spatial_id::RawSingleId;

#[tokio::test]
/// upsert (PATCH) を使用して、既存のデータを保持しつつ重なる部分以外を更新することを検証する
async fn test_layer_data_upsert_basic() {
    let test_app = TestApp::new();
    let layer_name = "upsert_layer";
    test_app.create_layer(layer_name, "Int", 25).await;

    // 1. Z=20 の領域 A にデータを挿入
    let query_a = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 100, "y": 100, "type": "singleId" }],
        "type": "spatialIds"
    });
    put_data(
        &test_app,
        layer_name,
        &serde_json::json!({ "value": 1, "query": query_a }),
    )
    .await;

    // 2. A と重なる親領域 B (Z=19) に対して upsert
    // upsert の場合、A の既存データは保持されたまま、B の残りの部分が埋まるはず
    let query_b = serde_json::json!({
        "ids": [{ "z": 19, "f": 0, "x": 50, "y": 50, "type": "singleId" }],
        "type": "spatialIds"
    });
    patch_data(
        &test_app,
        layer_name,
        &serde_json::json!({ "value": 10, "query": query_b }),
    )
    .await;

    // 3. 元々あった A の領域を検索
    // upsert なので値は 1 のままであるべき
    let res_a = search_data(&test_app, layer_name, &query_a).await;
    assert_first_entry(
        &res_a,
        1i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 100,
            y: 100,
        },
    );

    // 4. B の他の子ノードを検索
    // 新しく 10 が入っているはず
    let query_c = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 101, "y": 100, "type": "singleId" }],
        "type": "spatialIds"
    });
    let res_c = search_data(&test_app, layer_name, &query_c).await;
    assert_first_entry(
        &res_c,
        10i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 101,
            y: 100,
        },
    );
}
