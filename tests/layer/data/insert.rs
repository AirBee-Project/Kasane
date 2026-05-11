use std::collections::HashMap;

use crate::layer::common::TestApp;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use kasane::models::spatial_id::RawSingleId;
use kasane_logic::{IntoSingleIds, RangeId, SingleId};
use tower::ServiceExt;

#[tokio::test]
/// layerを作成して、空間IDと値が正しく挿入できているかどうかを検証する
async fn test_layer_data_insert_single_id() {
    let test_app = TestApp::new();

    // layerを作成する
    test_app.create_layer("test_layer", "Int", 25).await;

    //空間IDと値を挿入する
    let insert_body = serde_json::json!({
    "value": 3,
    "query": {
        "ids": [
            {
            "z": 20,
            "f": 0,
            "x": 931386,
            "y": 412905,
            "type": "singleId"
            },
        ],
        "type": "spatialIds"
        }
    });

    let req = Request::builder()
        .method("PUT")
        .uri("/layers/test_layer/data")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&insert_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    //同じ場所の値を取得する
    let get_body = serde_json::json!({
    "query": {
        "ids": [
            {
            "z": 20,
            "f": 0,
            "x": 931386,
            "y": 412905,
            "type": "singleId"
            },
        ],
        "type": "spatialIds"
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/layers/test_layer/data/search")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&get_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["ids"][0]["data"], 3);
    assert_eq!(body_json["ids"][0]["id"]["z"], 20);
    assert_eq!(body_json["ids"][0]["id"]["f"], 0);
    assert_eq!(body_json["ids"][0]["id"]["x"], 931386);
    assert_eq!(body_json["ids"][0]["id"]["y"], 412905);
}

#[tokio::test]
/// layerを作成して、空間IDと値が正しく挿入できているかどうかを検証する
async fn test_layer_data_insert_range_id() {
    let test_app = TestApp::new();

    // layerを作成する
    test_app.create_layer("test_layer", "Int", 25).await;

    //空間IDと値を挿入する
    let insert_body = serde_json::json!({
    "value": 3,
    "query": {
        "ids": [
            {
            "z": 20,
            "f": [0,100],
            "x": [931380,931386],
            "y": [412900,412905],
            "type": "rangeId"
            },
        ],
        "type": "spatialIds"
        }
    });

    let req = Request::builder()
        .method("PUT")
        .uri("/layers/test_layer/data")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&insert_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    //一部を取得する
    let get_body = serde_json::json!({
    "query": {
        "ids": [
            {
            "z": 20,
            "f": 0,
            "x": 931386,
            "y": 412905,
            "type": "singleId"
            },
        ],
        "type": "spatialIds"
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/layers/test_layer/data/search")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&get_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["ids"][0]["data"], 3);
    assert_eq!(body_json["ids"][0]["id"]["z"], 20);
    assert_eq!(body_json["ids"][0]["id"]["f"], 0);
    assert_eq!(body_json["ids"][0]["id"]["x"], 931386);
    assert_eq!(body_json["ids"][0]["id"]["y"], 412905);

    //全体を取得する
    let get_body = serde_json::json!({
    "query": {
        "ids": [
            {
            "z": 20,
            "f": [0,100],
            "x": [931380,931386],
            "y": [412900,412905],
            "type": "rangeId"
            },
        ],
        "type": "spatialIds"
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/layers/test_layer/data/search")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&get_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let mut result_map = HashMap::new();

    if let Some(ids) = body_json["ids"].as_array() {
        for item in ids {
            let id = &item["id"];
            let z = id["z"].as_i64().unwrap() as u8;
            let f = id["f"].as_i64().unwrap() as i32;
            let x = id["x"].as_u64().unwrap() as u32;
            let y = id["y"].as_u64().unwrap() as u32;
            let data = item["data"].as_i64().unwrap() as i32;
            result_map.insert(RawSingleId { z, f, x, y }, data);
        }
    }

    // 最適配置のSingleIdに分解すれば917個になるはず
    assert_eq!(result_map.len(), 917);

    //出力された結果が元のIDと一致しているか確認
    let mut answer: Vec<SingleId> = RangeId::new(20, [0, 100], [931380, 931386], [412900, 412905])
        .unwrap()
        .into_single_ids()
        .collect();

    let mut result: Vec<SingleId> = result_map
        .iter()
        .flat_map(|(raw_single_id, value)| {
            let single_id = SingleId::new(
                raw_single_id.z,
                raw_single_id.f,
                raw_single_id.x,
                raw_single_id.y,
            )
            .unwrap();

            assert_eq!(*value, 3);

            single_id
                .spatial_children_at_zoom(20)
                .unwrap()
                .collect::<Vec<_>>()
        })
        .collect();

    answer.sort();
    result.sort();

    assert_eq!(answer, result)
}
