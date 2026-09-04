use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{num, range_id, single_id};
use crate::common::data::put_data;

async fn get_table_count(test_app: &TestApp, table_name: &str) -> u64 {
    test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: table_name.to_string(),
        })
        .await
        .unwrap()
        .into_inner()
        .count
}

#[tokio::test]
/// データの挿入・更新・削除に伴い、テーブルの count が正しく増減するかを検証する。
async fn test_table_count_dynamic() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "count_test_table", "Int", 25)
        .await;

    // 初期状態の count は 0 であること
    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 0, "Initial count should be 0");

    // 1件目のデータを挿入
    let single_id_1 = single_id(20, 0, 100, 100);
    put_data(&test_app, "count_test_table", num(1.0), vec![single_id_1]).await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 1, "Count should be 1 after one insert");

    // 2件目のデータを挿入
    let single_id_2 = single_id(20, 0, 200, 200);
    put_data(&test_app, "count_test_table", num(2.0), vec![single_id_2]).await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 2, "Count should be 2 after second insert");

    // 既存のデータを上書き（count は 2 のまま変わらないこと）
    put_data(&test_app, "count_test_table", num(3.0), vec![single_id_2]).await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 2, "Count should remain 2 after overwrite");

    // range を使って範囲で挿入（例: z=21 の FlexId を 4 つ追加）
    let range = range_id(21, Some((0, 0)), Some((1000, 1001)), Some((1000, 1001)));
    put_data(&test_app, "count_test_table", num(4.0), vec![range]).await;

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(
        count, 3,
        "Count should be 3 after adding 4 flex_ids via range (merged into 1 parent block)"
    );

    // 1件目のデータを削除
    test_app
        .data()
        .remove(pb::RemoveDataRequest {
            db_name: "test_db".to_string(),
            table_name: "count_test_table".to_string(),
            spatial_ids: vec![single_id_1],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .unwrap();

    let count = get_table_count(&test_app, "count_test_table").await;
    assert_eq!(count, 2, "Count should be 2 after deleting 1 item");
}
