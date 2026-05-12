use std::collections::HashMap;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use kasane::models::spatial_id::RawSingleId;
use tower::ServiceExt;

use crate::layer::{
    common::TestApp,
    data::common::{assert_first_entry, put_data, search_data, to_result_map},
};

/// singleIdで指定した空間IDにデータを挿入し、削除できることを検証する
#[tokio::test]
async fn test_layer_data_remove_single_id() {
    let test_app = TestApp::new();
    test_app
        .create_layer("test_layer_BOOLEAN", "Boolean", 25)
        .await;

    let single_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    //挿入する
    put_data(
        &test_app,
        "test_layer_BOOLEAN",
        &serde_json::json!({ "value": true, "query": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_layer_BOOLEAN", &single_id_query).await;

    //値が正しく挿入できたか検証する
    assert_first_entry(
        &result_json,
        true,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );

    //削除する
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/layers/{}/data", "test_layer_BOOLEAN"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "query": single_id_query })).unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    //削除が完了する
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    //値が削除できていることを確認する
    let result_json = search_data(&test_app, "test_layer_BOOLEAN", &single_id_query).await;
    let result_map: HashMap<RawSingleId, bool> = to_result_map(&result_json);

    //値は空になっている
    assert!(result_map.is_empty());
}

#[tokio::test]
/// 親ノードが存在する領域の一部を削除したとき、その部分が正しく削除されることを検証する (Bug 3 の検証)
async fn test_layer_data_remove_logical_bug() {
    let test_app = TestApp::new();
    let layer_name = "bug3_layer";
    test_app.create_layer(layer_name, "Int", 25).await;

    // 1. Zoom 10 の広範な領域にデータを挿入 (これにより親ノードが作成される)
    let parent_id_query = serde_json::json!({
        "ids": [{ "z": 10, "f": 0, "x": 909, "y": 403, "type": "singleId" }],
        "type": "spatialIds"
    });
    put_data(
        &test_app,
        layer_name,
        &serde_json::json!({ "value": 100, "query": parent_id_query }),
    )
    .await;

    // 挿入されたことを確認
    let result = search_data(&test_app, layer_name, &parent_id_query).await;
    assert!(!to_result_map::<i64>(&result).is_empty());

    // 2. その領域内の Zoom 11 の子ノード部分を削除
    // (親ノード 10/0/909/403 の子は 11/0/1818/806, 11/0/1819/806, 11/0/1818/807, 11/0/1819/807)
    let child_id_query = serde_json::json!({
        "ids": [{ "z": 11, "f": 0, "x": 1818, "y": 806, "type": "singleId" }],
        "type": "spatialIds"
    });

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/layers/{}/data", layer_name))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "query": child_id_query })).unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 3. 削除した子ノード部分を検索
    // バグがある場合、削除したはずの領域にデータが再挿入されて残っている
    let result_json = search_data(&test_app, layer_name, &child_id_query).await;
    let result_map: HashMap<RawSingleId, i64> = to_result_map(&result_json);

    // 期待値は空だが、バグがあるとデータが残っている
    assert!(
        result_map.is_empty(),
        "Removed sub-area should be empty, but found: {:?}",
        result_map
    );
}
