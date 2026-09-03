use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::search_data;

/// Group Commit (WriteBatcher) が正しく複数リクエストを並行処理し、
/// 空間インデックスが破損せずに全データが保存されるかを検証する。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_table_data_concurrent_group_commit() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let num_tasks: u32 = 100;

    let mut tasks = vec![];

    // 100個のリクエストを同時に発行する
    for i in 0..num_tasks {
        let mut client = test_app.data();
        tasks.push(tokio::spawn(async move {
            let response = client
                .insert(pb::InsertDataRequest {
                    db_name: "test_db".to_string(),
                    table_name: "test_table".to_string(),
                    value: Some(num(i as f64)),
                    spatial_ids: vec![builders::single_id(20, 0, 931000 + i, 412000)],
                    zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
                })
                .await;
            assert!(response.is_ok());
        }));
    }

    for task in tasks {
        task.await.unwrap();
    }

    // データが全て正しく保存されているかを検証
    let query_all = vec![builders::range_id(
        20,
        Some((0, 0)),
        Some((931000, 931000 + num_tasks - 1)),
        Some((412000, 412000)),
    )];

    let result = search_data(&test_app, "test_table", query_all).await;

    // 全ての値が存在することを確認
    let mut found_values = vec![false; num_tasks as usize];

    // 合計の spatial IDs 数を数える
    let mut total_ids = 0;

    for group in &result.data {
        let value = result
            .dictionary
            .get(group.value_ref as usize)
            .expect("value_ref out of range");
        let actual_data = builders::value_as_f64(value).expect("expected a number") as usize;
        assert!(actual_data < num_tasks as usize, "Unexpected value found");
        found_values[actual_data] = true;

        total_ids += group.spatial_ids.len();
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
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let num_valid: u32 = 50;

    let mut tasks = vec![];

    for i in 0..num_valid {
        let mut client = test_app.data();
        // 正常リクエスト（Int）と、その合間に不正リクエスト（Int テーブルへ文字列）を混ぜる。
        tasks.push(tokio::spawn(async move {
            let make_req = |value: prost_types::Value, x: u32| pb::InsertDataRequest {
                db_name: "test_db".to_string(),
                table_name: "test_table".to_string(),
                value: Some(value),
                spatial_ids: vec![builders::single_id(20, 0, x, 413000)],
                zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            };

            let valid_res = client.insert(make_req(num(i as f64), 941000 + i)).await;
            assert!(
                valid_res.is_ok(),
                "valid insert was rejected (batch abort regression?)"
            );

            let invalid_res = client
                .insert(make_req(builders::text("not_an_int"), 941000 + i))
                .await;
            match invalid_res {
                Err(status) => assert_eq!(
                    status.code(),
                    tonic::Code::InvalidArgument,
                    "invalid insert should fail with InvalidArgument, got {status:?}"
                ),
                Ok(_) => panic!("invalid insert should fail with a client error"),
            }
        }));
    }

    for task in tasks {
        task.await.unwrap();
    }

    // 正常に投入した num_valid 件がすべて保存されていること。
    let query_all = vec![builders::range_id(
        20,
        Some((0, 0)),
        Some((941000, 941000 + num_valid - 1)),
        Some((413000, 413000)),
    )];

    let result = search_data(&test_app, "test_table", query_all).await;

    let mut total_ids = 0;
    for group in &result.data {
        total_ids += group.spatial_ids.len();
    }
    assert_eq!(
        total_ids, num_valid as usize,
        "some valid inserts were lost — invalid requests aborted the batch"
    );
}
