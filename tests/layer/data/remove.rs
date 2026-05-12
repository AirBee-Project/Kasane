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
