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
        .create_table("test_db", "test_table", "Int", 25)
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
