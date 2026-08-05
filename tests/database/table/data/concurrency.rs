use axum::body::Body;
use axum::http::{Request, StatusCode, header};

use tower::ServiceExt;

use crate::database::table::common::TestApp;
use crate::database::table::data::common::search_data;

/// Group Commit (WriteBatcher) が正しく複数リクエストを並行処理し、
/// 空間インデックスが破損せずに全データが保存されるかを検証する。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_table_data_concurrent_group_commit() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int")
        .await;

    let app = test_app.app.clone();
    let num_tasks = 100;

    let mut tasks = vec![];

    // 100個のリクエストを同時に発行する
    for i in 0..num_tasks {
        let app_clone = app.clone();
        tasks.push(tokio::spawn(async move {
            let single_id_query = serde_json::json!([{
                "z": 20,
                "f": 0,
                "x": 931000 + i,
                "y": 412000,
                "type": "singleId"
            }]);

            let body = serde_json::json!({
                "value": i,
                "spatial_ids": single_id_query
            });

            let req = Request::builder()
                .method("PUT")
                .uri("/databases/test_db/tables/test_table/data")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap();

            let response = app_clone.oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }));
    }

    for task in tasks {
        task.await.unwrap();
    }

    // データが全て正しく保存されているかを検証
    let query_all = serde_json::json!([{
        "z": 20,
        "f": [0, 0],
        "x": [931000, 931000 + num_tasks - 1],
        "y": [412000, 412000],
        "type": "rangeId"
    }]);

    let result_json = search_data(&test_app, "test_table", &query_all).await;

    let dict = result_json["dictionary"].as_array().expect("No dictionary");
    let data = result_json["data"].as_array().expect("No data");

    // 全ての値が存在することを確認
    let mut found_values = vec![false; num_tasks as usize];

    // 合計の spatial IDs 数を数える
    let mut total_ids = 0;

    for group in data {
        let value_ref = group["valueRef"].as_u64().unwrap() as usize;
        let actual_data = dict[value_ref].as_i64().unwrap() as usize;
        assert!(actual_data < num_tasks as usize, "Unexpected value found");
        found_values[actual_data] = true;

        let spatial_ids = group["spatialIds"].as_array().expect("No spatialIds");
        total_ids += spatial_ids.len();
    }

    assert_eq!(
        total_ids, num_tasks as usize,
        "Not all inserted items were found"
    );
    assert!(found_values.iter().all(|&v| v), "Some values were missing");
}

/// 不正なリクエスト（型不一致）が、同時刻の正常なリクエストと同じ書き込みバッチに
/// 入っても、正常なリクエストを巻き添えでロールバックさせないことを検証する。
///
/// 検証（テーブル存在・値の解釈）は enqueue 前に完了するため、不正リクエストは
/// そもそもバッチへ投入されず、正常リクエストは全て commit される。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_invalid_request_does_not_abort_valid_batch() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int")
        .await;

    let app = test_app.app.clone();
    let num_valid = 50;

    let mut tasks = vec![];

    for i in 0..num_valid {
        let app_clone = app.clone();
        // 正常リクエスト（Int）と、その合間に不正リクエスト（Int テーブルへ文字列）を混ぜる。
        let valid_value = serde_json::json!(i);
        let invalid_value = serde_json::json!("not_an_int");

        tasks.push(tokio::spawn(async move {
            let make_req = |value: serde_json::Value, x: i64| {
                let body = serde_json::json!({
                    "value": value,
                    "spatial_ids": serde_json::json!([{
                        "z": 20, "f": 0, "x": x, "y": 413000, "type": "singleId"
                    }])
                });
                Request::builder()
                    .method("PUT")
                    .uri("/databases/test_db/tables/test_table/data")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap()
            };

            let valid_res = app_clone
                .clone()
                .oneshot(make_req(valid_value, 941000 + i))
                .await
                .unwrap();
            assert_eq!(
                valid_res.status(),
                StatusCode::OK,
                "valid insert was rejected (batch abort regression?)"
            );

            let invalid_res = app_clone
                .oneshot(make_req(invalid_value, 941000 + i))
                .await
                .unwrap();
            assert!(
                invalid_res.status().is_client_error(),
                "invalid insert should fail with a client error, got {}",
                invalid_res.status()
            );
        }));
    }

    for task in tasks {
        task.await.unwrap();
    }

    // 正常に投入した num_valid 件がすべて保存されていること。
    let query_all = serde_json::json!([{
        "z": 20,
        "f": [0, 0],
        "x": [941000, 941000 + num_valid - 1],
        "y": [413000, 413000],
        "type": "rangeId"
    }]);

    let result_json = search_data(&test_app, "test_table", &query_all).await;
    let data = result_json["data"].as_array().expect("No data");

    let mut total_ids = 0;
    for group in data {
        total_ids += group["spatialIds"].as_array().expect("No spatialIds").len();
    }
    assert_eq!(
        total_ids, num_valid as usize,
        "some valid inserts were lost — invalid requests aborted the batch"
    );
}
