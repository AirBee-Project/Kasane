use std::collections::HashMap;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use kasane::models::spatial_id::RawSingleId;
use tower::ServiceExt;

use crate::database::table::{
    common::TestApp,
    data::common::{assert_first_entry, put_data, search_data, to_result_map},
};

/// singleIdで指定した空間IDのデータを挿入後に正常に削除できるかを検証する。
#[tokio::test]
async fn test_table_data_remove_single_id() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table_BOOLEAN", "Boolean", 25)
        .await;

    let single_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_table_BOOLEAN",
        &serde_json::json!({ "value": true, "query": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table_BOOLEAN", &single_id_query).await;

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

    let req = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/databases/test_db/tables/{}/data",
            "test_table_BOOLEAN"
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "query": single_id_query })).unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let result_json = search_data(&test_app, "test_table_BOOLEAN", &single_id_query).await;
    let result_map: HashMap<RawSingleId, bool> = to_result_map(&result_json);

    assert!(result_map.is_empty());
}

#[tokio::test]
/// 親ノードが存在する領域の一部を削除した際、その部分のみが正しく削除されるかを検証する。
async fn test_table_data_remove_logical_bug() {
    let test_app = TestApp::new();

    let table_name = "bug3_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let parent_id_query = serde_json::json!({
        "ids": [{ "z": 10, "f": 0, "x": 909, "y": 403, "type": "singleId" }],
        "type": "spatialIds"
    });
    put_data(
        &test_app,
        table_name,
        &serde_json::json!({ "value": 100, "query": parent_id_query }),
    )
    .await;

    let result = search_data(&test_app, table_name, &parent_id_query).await;
    assert!(!to_result_map::<i64>(&result).is_empty());

    let child_id_query = serde_json::json!({
        "ids": [{ "z": 11, "f": 0, "x": 1818, "y": 806, "type": "singleId" }],
        "type": "spatialIds"
    });

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/databases/test_db/tables/{}/data", table_name))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({ "query": child_id_query })).unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let result_json = search_data(&test_app, table_name, &child_id_query).await;
    let result_map: HashMap<RawSingleId, i64> = to_result_map(&result_json);

    assert!(
        result_map.is_empty(),
        "Removed sub-area should be empty, but found: {:?}",
        result_map
    );
}

#[tokio::test]
/// 存在するデータの一部のみが削除クエリの範囲に含まれる場合、重なっている部分だけが削除されるかを検証する。
async fn test_table_data_remove_partial_overlap() {
    let test_app = TestApp::new();

    let table_name = "partial_remove_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let id1 = serde_json::json!({ "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" });
    let id2 = serde_json::json!({ "z": 20, "f": 0, "x": 11, "y": 10, "type": "singleId" });

    let insert_query = serde_json::json!({
        "ids": [id1, id2],
        "type": "spatialIds"
    });
    put_data(
        &test_app,
        table_name,
        &serde_json::json!({ "value": 500, "query": insert_query }),
    )
    .await;

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/databases/test_db/tables/{}/data", table_name))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "query": { "ids": [id1], "type": "spatialIds" }
            }))
            .unwrap(),
        ))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let res1 = search_data(
        &test_app,
        table_name,
        &serde_json::json!({ "ids": [id1], "type": "spatialIds" }),
    )
    .await;
    assert!(to_result_map::<i64>(&res1).is_empty());

    let res2 = search_data(
        &test_app,
        table_name,
        &serde_json::json!({ "ids": [id2], "type": "spatialIds" }),
    )
    .await;
    assert_first_entry(
        &res2,
        500i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 11,
            y: 10,
        },
    );
}
