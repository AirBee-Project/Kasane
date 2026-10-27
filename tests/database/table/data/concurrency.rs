use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::{group_value, search_data};

/// 同時に届いた書き込みがすべて保存され、空間インデックスが壊れないことを検証する。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_table_data_concurrent_writes() {
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
                .insert(pb::WriteDataRequest {
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
        let value = group_value(&result.dictionary, group);
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
