use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use kasane::models::spatial_id::RawSingleId;
use kasane_logic::{IterSingleIds, RangeId, SingleId};
use tower::ServiceExt;

use crate::database::table::common::TestApp;
use crate::database::table::data::common::{
    assert_first_entry, put_data, search_data, to_result_map,
};

/// singleIdで指定した空間IDにデータを挿入し、同じ場所から正しく取得できるかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 3, "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table", &single_id_query).await;

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

/// TinyInt型のデータ挿入およびその範囲外の値がバリデーションエラーになるかを検証する。
#[tokio::test]
async fn test_table_data_insert_tinyint() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "TinyInt", 25)
        .await;

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    // Valid value
    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 127, "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table", &single_id_query).await;
    assert_first_entry(
        &result_json,
        127i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );

    // Invalid value (Out of range)
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/test_table/data")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": 128, "spatial_ids": single_id_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Double型のデータ挿入および取得が正常に行えるかを検証する。
#[tokio::test]
async fn test_table_data_insert_double() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Double", 25)
        .await;

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 9.99, "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table", &single_id_query).await;
    assert_first_entry(
        &result_json,
        9.99f64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
        },
    );
}

/// singleIdで指定した空間IDに、テーブルの型と一致しない値を挿入した際にエラーが返るかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id_error() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/databases/test_db/tables/{}/data", "test_table"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": "SampleText", "spatial_ids": single_id_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 不正なsingleIdを入力した際にエラーが返るかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id_logic_error() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Text", 25)
        .await;

    let single_id_query =
        serde_json::json!([{ "z": 3, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/databases/test_db/tables/{}/data", "test_table"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": "SampleText", "spatial_ids": single_id_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// 2つのsingleIdに対してそれぞれデータが正しく挿入できるかを検証する。
#[tokio::test]
async fn test_table_data_insert_two_single_id() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query_1 =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 3, "spatial_ids": single_id_query_1 }),
    )
    .await;

    let single_id_query_2 =
        serde_json::json!([{ "z": 20, "f": -1, "x": 931386, "y": 412905, "type": "singleId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 4, "spatial_ids": single_id_query_2 }),
    )
    .await;

    let result_json_1 = search_data(&test_app, "test_table", &single_id_query_1).await;
    let result_json_2 = search_data(&test_app, "test_table", &single_id_query_2).await;

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

/// 同じsingleIdに対してデータを挿入した場合、値が正しく上書きされるかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id_overwrite() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 3, "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table", &single_id_query).await;

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

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 4, "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table", &single_id_query).await;

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

/// 同じrangeIdに対してデータを挿入した場合、値が正しく上書きされるかを検証する。
#[tokio::test]
async fn test_table_data_insert_range_id_overwrite() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table_text", "Text", 25)
        .await;

    let range_id_query = serde_json::json!([{ "z": 18, "f": [0,0], "x": [232846,232850], "y": [103226,103240], "type": "rangeId" }]);

    put_data(
        &test_app,
        "test_table_text",
        &serde_json::json!({ "value": "猫(Cat)", "spatial_ids": range_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table_text", &range_id_query).await;
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

    put_data(
        &test_app,
        "test_table_text",
        &serde_json::json!({ "value": "犬(Dog)", "spatial_ids": range_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table_text", &range_id_query).await;
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

/// rangeIdで指定した範囲にデータを挿入し、一部・全体それぞれが正しく取得できるかを検証する。
#[tokio::test]
async fn test_table_data_insert_range_id() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let range_id_query = serde_json::json!([{ "z": 20, "f": [0, 100], "x": [931380, 931386], "y": [412900, 412905], "type": "rangeId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 3, "spatial_ids": range_id_query }),
    )
    .await;

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);
    let result_json = search_data(&test_app, "test_table", &single_id_query).await;

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

    let result_json = search_data(&test_app, "test_table", &range_id_query).await;
    let result_map = to_result_map::<i64>(&result_json);

    assert_eq!(result_map.len(), 917);

    let mut answer: Vec<SingleId> = RangeId::new(20, [0, 100], [931380, 931386], [412900, 412905])
        .unwrap()
        .iter_single_ids()
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

/// Insertを用いて一部の値の上書きを行った際、新しい値と元の値が正しい状態を保つかを検証する。
#[tokio::test]
async fn test_table_data_overload_insert() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Text", 30)
        .await;

    let query1 =
        serde_json::json!([{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": "A", "spatial_ids": query1 }),
    )
    .await;

    let query2 =
        serde_json::json!([{ "z": 21, "f": 0, "x": 1862772, "y": 825810, "type": "singleId" }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": "B", "spatial_ids": query2 }),
    )
    .await;

    let result_json = search_data(&test_app, "test_table", &query1).await;
    let result_map = to_result_map::<String>(&result_json);

    assert_eq!(result_map.len(), 8);

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

#[tokio::test]
/// 64個のノード（Zoom 20）を順次挿入した際、再帰的にマージされて1つのZoom 18ノードになるかを検証する。
async fn test_table_data_recursive_merge() {
    let test_app = TestApp::new();

    let table_name = "merge_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    for f in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                let single_id_query =
                    serde_json::json!([{ "z": 20, "f": f, "x": x, "y": y, "type": "singleId" }]);
                put_data(
                    &test_app,
                    table_name,
                    &serde_json::json!({ "value": 7, "spatial_ids": single_id_query }),
                )
                .await;
            }
        }
    }

    let search_query = serde_json::json!([{ "z": 18, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);
    let result_json = search_data(&test_app, table_name, &search_query).await;
    let result_map = to_result_map::<i64>(&result_json);

    assert_eq!(
        result_map.len(),
        1,
        "Should be merged into a single node, but found: {:?}",
        result_map
    );

    let (raw_id, &value) = result_map.iter().next().unwrap();
    assert_eq!(raw_id.z, 18);
    assert_eq!(raw_id.f, 0);
    assert_eq!(raw_id.x, 0);
    assert_eq!(raw_id.y, 0);
    assert_eq!(value, 7);
}

#[tokio::test]
/// 異なるテーブル間で同じ座標にデータを挿入しても、互いに干渉しないかを検証する。
async fn test_table_data_isolation() {
    let test_app = TestApp::new();

    let table1 = "table1";
    let table2 = "table2";

    test_app.create_database("test_db").await;
    test_app.create_table("test_db", table1, "Int", 25).await;
    test_app.create_table("test_db", table2, "Int", 25).await;

    let query = serde_json::json!([{ "z": 20, "f": 0, "x": 100, "y": 100, "type": "singleId" }]);

    put_data(
        &test_app,
        table1,
        &serde_json::json!({ "value": 1, "spatial_ids": query }),
    )
    .await;
    put_data(
        &test_app,
        table2,
        &serde_json::json!({ "value": 2, "spatial_ids": query }),
    )
    .await;

    let res1 = search_data(&test_app, table1, &query).await;
    assert_first_entry(
        &res1,
        1i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 100,
            y: 100,
        },
    );

    let res2 = search_data(&test_app, table2, &query).await;
    assert_first_entry(
        &res2,
        2i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 100,
            y: 100,
        },
    );
}

#[tokio::test]
/// max_zoom_levelを超えるズームレベルでの挿入がエラーになるかを検証する。
async fn test_table_data_max_zoom_enforcement() {
    let test_app = TestApp::new();

    let table_name = "low_zoom_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 10)
        .await;

    let high_zoom_query =
        serde_json::json!([{ "z": 11, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/databases/test_db/tables/{}/data", table_name))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": 100, "spatial_ids": high_zoom_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
/// 広範な親ノード内にピンポイントな子ノードを挿入した際、親が適切に分割され値の整合性が保たれるかを検証する。
async fn test_table_data_deep_split() {
    let test_app = TestApp::new();

    let table_name = "split_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let parent_query = serde_json::json!([{ "z": 18, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);
    put_data(
        &test_app,
        table_name,
        &serde_json::json!({ "value": 100, "spatial_ids": parent_query }),
    )
    .await;

    let child_query = serde_json::json!([{ "z": 20, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);
    put_data(
        &test_app,
        table_name,
        &serde_json::json!({ "value": 200, "spatial_ids": child_query }),
    )
    .await;

    let res_child = search_data(&test_app, table_name, &child_query).await;
    assert_first_entry(
        &res_child,
        200i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 0,
            y: 0,
        },
    );

    let sibling_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 1, "y": 0, "type": "singleId" }]);
    let res_sibling = search_data(&test_app, table_name, &sibling_query).await;
    assert_first_entry(
        &res_sibling,
        100i64,
        RawSingleId {
            z: 20,
            f: 0,
            x: 1,
            y: 0,
        },
    );
}

#[tokio::test]
/// Enum型のテーブルに対して、許可された値の挿入が成功することを検証する。
async fn test_table_data_insert_enum_success() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "enum_table",
        "data_type": "Enum",
        "max_zoom_level": 25,
        "constraints": {
            "type": "Enum",
            "choices": ["Apple", "Banana", "Orange"]
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    test_app.app.clone().oneshot(req).await.unwrap();

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);

    put_data(
        &test_app,
        "enum_table",
        &serde_json::json!({ "value": "Banana", "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "enum_table", &single_id_query).await;
    assert_first_entry(
        &result_json,
        "Banana".to_string(),
        RawSingleId {
            z: 20,
            f: 0,
            x: 0,
            y: 0,
        },
    );
}

#[tokio::test]
/// Enum型のテーブルに対して、許可されていない値の挿入が失敗することを検証する。
async fn test_table_data_insert_enum_failure() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "enum_table",
        "data_type": "Enum",
        "max_zoom_level": 25,
        "constraints": {
            "type": "Enum",
            "choices": ["Apple", "Banana", "Orange"]
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    test_app.app.clone().oneshot(req).await.unwrap();

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/databases/test_db/tables/{}/data", "enum_table"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": "Grape", "spatial_ids": single_id_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
/// Presence型のテーブルに対して、null の挿入が成功することを検証する。
async fn test_table_data_insert_presence_success() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "presence_table",
        "data_type": "Presence",
        "max_zoom_level": 25
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    test_app.app.clone().oneshot(req).await.unwrap();

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);

    put_data(
        &test_app,
        "presence_table",
        &serde_json::json!({ "value": null, "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = search_data(&test_app, "presence_table", &single_id_query).await;

    assert_first_entry(
        &result_json,
        serde_json::Value::Null,
        RawSingleId {
            z: 20,
            f: 0,
            x: 0,
            y: 0,
        },
    );
}

#[tokio::test]
/// Presence型のテーブルに対して、null 以外の値の挿入が失敗することを検証する。
async fn test_table_data_insert_presence_failure() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "presence_table",
        "data_type": "Presence",
        "max_zoom_level": 25
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    test_app.app.clone().oneshot(req).await.unwrap();

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);

    let req = Request::builder()
        .method("PUT")
        .uri(format!(
            "/databases/test_db/tables/{}/data",
            "presence_table"
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(
                &serde_json::json!({ "value": "some_value", "spatial_ids": single_id_query }),
            )
            .unwrap(),
        ))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
