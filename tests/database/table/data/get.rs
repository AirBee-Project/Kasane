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
        .create_table("test_db", table_name, "Int")
        .await;

    let id1 = RawSingleId {
        z: 20,
        f: 0,
        x: 10,
        y: 10,
        i: None,
        t: None,
    };
    let id2 = RawSingleId {
        z: 20,
        f: 0,
        x: 20,
        y: 20,
        i: None,
        t: None,
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

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
/// RangeIdとFlexIdでのレスポンスフォーマットを検証する。
async fn test_table_data_get_format_options() {
    let test_app = TestApp::new();

    let table_name = "get_table_formats";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int")
        .await;

    put_data(
        &test_app,
        table_name,
        &serde_json::json!({
            "value": 500,
            "spatial_ids": [{ "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" }]
        }),
    )
    .await;

    let query = serde_json::json!([
        { "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" }
    ]);

    let body = serde_json::json!({ "spatial_ids": query });

    // Test RangeId
    let req_range = Request::builder()
        .method("POST")
        .uri(format!(
            "/databases/test_db/tables/{}/data/search?format=rangeId",
            table_name
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let res_range = test_app.app.clone().oneshot(req_range).await.unwrap();
    assert_eq!(res_range.status(), StatusCode::OK);
    let bytes_range = res_range.into_body().collect().await.unwrap().to_bytes();
    let json_range: serde_json::Value = serde_json::from_slice(&bytes_range).unwrap();

    let spatial_ids = json_range["data"][0]["spatialIds"].as_array().unwrap();
    assert!(spatial_ids[0].get("z").is_some());
    assert!(spatial_ids[0].get("f").is_some());
    assert!(spatial_ids[0].get("x").is_some());
    assert!(spatial_ids[0].get("y").is_some());

    // Test FlexId
    let req_flex = Request::builder()
        .method("POST")
        .uri(format!(
            "/databases/test_db/tables/{}/data/search?format=flexId",
            table_name
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let res_flex = test_app.app.clone().oneshot(req_flex).await.unwrap();
    assert_eq!(res_flex.status(), StatusCode::OK);
    let bytes_flex = res_flex.into_body().collect().await.unwrap().to_bytes();
    let json_flex: serde_json::Value = serde_json::from_slice(&bytes_flex).unwrap();

    let spatial_ids_flex = json_flex["data"][0]["spatialIds"].as_array().unwrap();
    assert!(spatial_ids_flex[0].get("fZoomlevel").is_some());
    assert!(spatial_ids_flex[0].get("fIndex").is_some());
    assert!(spatial_ids_flex[0].get("xZoomlevel").is_some());
    assert!(spatial_ids_flex[0].get("xIndex").is_some());
    assert!(spatial_ids_flex[0].get("yZoomlevel").is_some());
    assert!(spatial_ids_flex[0].get("yIndex").is_some());
}
