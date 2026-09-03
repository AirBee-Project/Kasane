use std::collections::HashMap;

use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, boolean, num};
use crate::common::data::{assert_first_entry, put_data, search_data, to_result_map};

async fn remove_data(test_app: &TestApp, table_name: &str, spatial_ids: Vec<pb::SpatialId>) {
    test_app
        .data()
        .remove(pb::RemoveDataRequest {
            db_name: "test_db".to_string(),
            table_name: table_name.to_string(),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .expect("remove failed");
}

/// singleIdで指定した空間IDのデータを挿入後に正常に削除できるかを検証する。
#[tokio::test]
async fn test_table_data_remove_single_id() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table_BOOLEAN", "Boolean", 25)
        .await;

    let single_id_query = vec![builders::single_id(20, 0, 931386, 412905)];

    put_data(
        &test_app,
        "test_table_BOOLEAN",
        boolean(true),
        single_id_query.clone(),
    )
    .await;

    let result = search_data(&test_app, "test_table_BOOLEAN", single_id_query.clone()).await;

    assert_first_entry(
        &result,
        &boolean(true),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );

    remove_data(&test_app, "test_table_BOOLEAN", single_id_query.clone()).await;

    let result = search_data(&test_app, "test_table_BOOLEAN", single_id_query).await;
    let result_map: HashMap<(u32, i32, u32, u32), bool> = to_result_map(&result);

    assert!(result_map.is_empty());
}

#[tokio::test]
/// 親ノードが存在する領域の一部を削除した際、その部分のみが正しく削除されるかを検証する。
async fn test_table_data_remove_logical_bug() {
    let test_app = TestApp::new().await;

    let table_name = "bug3_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let parent_id_query = vec![builders::single_id(10, 0, 909, 403)];
    put_data(&test_app, table_name, num(100.0), parent_id_query.clone()).await;

    let result = search_data(&test_app, table_name, parent_id_query).await;
    assert!(!to_result_map::<i64>(&result).is_empty());

    let child_id_query = vec![builders::single_id(11, 0, 1818, 806)];

    remove_data(&test_app, table_name, child_id_query.clone()).await;

    let result = search_data(&test_app, table_name, child_id_query).await;
    let result_map: HashMap<(u32, i32, u32, u32), i64> = to_result_map(&result);

    assert!(
        result_map.is_empty(),
        "Removed sub-area should be empty, but found: {:?}",
        result_map
    );
}

#[tokio::test]
/// 存在するデータの一部のみが削除クエリの範囲に含まれる場合、重なっている部分だけが削除されるかを検証する。
async fn test_table_data_remove_partial_overlap() {
    let test_app = TestApp::new().await;

    let table_name = "partial_remove_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let id1 = builders::single_id(20, 0, 10, 10);
    let id2 = builders::single_id(20, 0, 11, 10);

    let insert_query = vec![id1, id2];
    put_data(&test_app, table_name, num(500.0), insert_query).await;

    remove_data(&test_app, table_name, vec![id1]).await;

    let res1 = search_data(&test_app, table_name, vec![id1]).await;
    assert!(to_result_map::<i64>(&res1).is_empty());

    let res2 = search_data(&test_app, table_name, vec![id2]).await;
    assert_first_entry(
        &res2,
        &num(500.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 11,
            y: 10,
            i: None,
            t: None,
        },
    );
}
