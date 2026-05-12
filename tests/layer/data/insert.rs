use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use kasane::models::spatial_id::RawSingleId;
use kasane_logic::{IntoSingleIds, IterSingleIds, RangeId, SingleId};
use tower::ServiceExt;

use crate::layer::common::TestApp;
use crate::layer::data::common::{assert_first_entry, put_data, search_data, to_result_map};

/// singleIdで指定した空間IDにデータを挿入し、同じ場所から正しく取得できるか検証する
#[tokio::test]
async fn test_layer_data_insert_single_id() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer", "Int", 25).await;

    let single_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": 3, "query": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_layer", &single_id_query).await;

    assert_first_entry(
        &result_json,
        3i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );
}

/// singleIdで指定した空間IDにlayerと型が一致しない値を挿入し、エラーが帰ってくることを検証する
#[tokio::test]
async fn test_layer_data_insert_single_id_error() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer", "Int", 25).await;

    let single_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/layers/{}/data", "test_layer"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": "SampleText", "query": single_id_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    //エラーが帰ってくる
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 間違ったSingleIdを入力したときにエラーが帰ってくることを確認する
#[tokio::test]
async fn test_layer_data_insert_single_id_logic_error() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer", "Text", 25).await;

    let single_id_query = serde_json::json!({
        "ids": [{ "z": 3, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/layers/{}/data", "test_layer"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": "SampleText", "query": single_id_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    //エラーが帰ってくる
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 2つのSingleIdを挿入したときに正しく挿入できることを検証する
#[tokio::test]
async fn test_layer_data_insert_two_single_id() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer", "Int", 25).await;

    //1つ目のSingleIdを挿入する
    let single_id_query_1 = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": 3, "query": single_id_query_1 }),
    )
    .await;

    //2つ目のSingleIdを挿入する
    let single_id_query_2 = serde_json::json!({
        "ids": [{ "z": 20, "f": -1, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": 4, "query": single_id_query_2 }),
    )
    .await;

    let result_json_1 = search_data(&test_app, "test_layer", &single_id_query_1).await;
    let result_json_2 = search_data(&test_app, "test_layer", &single_id_query_2).await;

    assert_first_entry(
        &result_json_1,
        3i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );

    assert_first_entry(
        &result_json_2,
        4i64,
        RawSingleId {
            z: 20,
            f: -1,
            x: 931386,
            y: 412905,
        },
    );
}

/// 同じSingleIdに値を挿入して上書きできていることを検証する
#[tokio::test]
async fn test_layer_data_insert_single_id_overwrite() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer", "Int", 25).await;

    //1つ目のSingleIdを挿入する
    let single_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": 3, "query": single_id_query }),
    )
    .await;

    //1つ目のSingleIdが正しく入力されていることを検証する
    let result_json = search_data(&test_app, "test_layer", &single_id_query).await;

    assert_first_entry(
        &result_json,
        3i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );

    //2つ目のSingleIdを挿入する
    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": 4, "query": single_id_query }),
    )
    .await;

    //2つ目のSingleIdが正しく入力されていることを検証する
    let result_json = search_data(&test_app, "test_layer", &single_id_query).await;

    assert_first_entry(
        &result_json,
        4i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );
}

/// 同じRangeIdに値を挿入して上書きできていることを検証する
#[tokio::test]
async fn test_layer_data_insert_range_id_overwrite() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer_text", "Text", 25).await;

    //1つ目のRangeIdを挿入する
    let range_id_query = serde_json::json!({
        "ids": [{ "z": 18, "f": [0,0], "x": [232846,232850], "y": [103226,103240], "type": "rangeId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer_text",
        &serde_json::json!({ "value": "猫(Cat)", "query": range_id_query }),
    )
    .await;

    //1つ目のRangeIdに値が挿入できていることを確認する
    let result_json = search_data(&test_app, "test_layer_text", &range_id_query).await;
    let result_map: std::collections::HashMap<RawSingleId, String> = to_result_map(&result_json);

    let mut result: Vec<SingleId> = result_map
        .iter()
        .flat_map(|(raw_id, value)| {
            assert_eq!(value, "猫(Cat)");
            SingleId::new(raw_id.z, raw_id.f, raw_id.x, raw_id.y)
                .unwrap()
                .spatial_children_at_zoom(18)
                .unwrap()
                .collect::<Vec<_>>()
        })
        .collect();
    let binding = RangeId::new(18, [0, 0], [232846, 232850], [103226, 103240]).unwrap();
    let mut answer: Vec<SingleId> = binding.iter_single_ids().collect();

    answer.sort();
    result.sort();

    assert_eq!(answer, result);

    //2つ目のRangeIdを挿入する
    put_data(
        &test_app,
        "test_layer_text",
        &serde_json::json!({ "value": "犬(Dog)", "query": range_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_layer_text", &range_id_query).await;
    let result_map: std::collections::HashMap<RawSingleId, String> = to_result_map(&result_json);

    let mut result: Vec<SingleId> = result_map
        .iter()
        .flat_map(|(raw_id, value)| {
            assert_eq!(value, "犬(Dog)");
            SingleId::new(raw_id.z, raw_id.f, raw_id.x, raw_id.y)
                .unwrap()
                .spatial_children_at_zoom(18)
                .unwrap()
                .collect::<Vec<_>>()
        })
        .collect();
    let binding = RangeId::new(18, [0, 0], [232846, 232850], [103226, 103240]).unwrap();
    let mut answer: Vec<SingleId> = binding.iter_single_ids().collect();

    answer.sort();
    result.sort();

    assert_eq!(answer, result);
}

/// rangeIdで指定した範囲にデータを挿入し、一部・全体それぞれが正しく取得できるか検証する
#[tokio::test]
async fn test_layer_data_insert_range_id() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer", "Int", 25).await;

    let range_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": [0, 100], "x": [931380, 931386], "y": [412900, 412905], "type": "rangeId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": 3, "query": range_id_query }),
    )
    .await;

    // 範囲内の一点だけを取得して検証する
    let single_id_query = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });
    let result_json = search_data(&test_app, "test_layer", &single_id_query).await;

    assert_first_entry(
        &result_json,
        3i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );

    // 範囲全体を取得して、件数とSingleIdへの分解結果を検証する
    let result_json = search_data(&test_app, "test_layer", &range_id_query).await;
    let result_map = to_result_map::<i64>(&result_json);

    // 最適配置のSingleIdに分解すれば917個になるはず
    assert_eq!(result_map.len(), 917);

    // 各エントリの値が正しく、SpatialChildrenへの展開がanswerと一致するか検証する
    let mut answer: Vec<SingleId> = RangeId::new(20, [0, 100], [931380, 931386], [412900, 412905])
        .unwrap()
        .into_single_ids()
        .collect();

    let mut result: Vec<SingleId> = result_map
        .iter()
        .flat_map(|(raw_id, &value)| {
            assert_eq!(value, 3);
            SingleId::new(raw_id.z, raw_id.f, raw_id.x, raw_id.y)
                .unwrap()
                .spatial_children_at_zoom(20)
                .unwrap()
                .collect::<Vec<_>>()
        })
        .collect();

    answer.sort();
    result.sort();
    assert_eq!(answer, result);
}

/// Insertを用いて一部の値の上書きを行ったときに、新しい値と元の値が正しい状態を保つことを検証する
#[tokio::test]
async fn test_layer_data_overload_insert() {
    let test_app = TestApp::new();
    test_app.create_layer("test_layer", "Text", 30).await;

    let query1 = serde_json::json!({
        "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": "A", "query": query1 }),
    )
    .await;

    let query2 = serde_json::json!({
        "ids": [{ "z": 21, "f": 0, "x": 1862772, "y": 825810, "type": "singleId" }],
        "type": "spatialIds"
    });

    put_data(
        &test_app,
        "test_layer",
        &serde_json::json!({ "value": "B", "query": query2 }),
    )
    .await;

    let result_json = search_data(&test_app, "test_layer", &query1).await;
    let result_map = to_result_map::<String>(&result_json);

    //SingleIdの個数は8個なはず
    assert_eq!(result_map.len(), 8);

    //上書きした部分
    let overload_single_id = RawSingleId {
        z: 21,
        f: 0,
        x: 1862772,
        y: 825810,
    };

    for (raw_single_id, value) in result_map {
        if raw_single_id == overload_single_id {
            assert_eq!(value, "B".to_string());
        } else {
            assert_eq!(value, "A".to_string());
        }
    }
}
