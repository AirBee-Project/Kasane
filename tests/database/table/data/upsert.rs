use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::{assert_first_entry, patch_data, put_data, search_data};

#[tokio::test]
/// upsert (PATCH) により、既存データを保持しつつ重なる部分以外が正しく更新されるかを検証する。
async fn test_table_data_upsert_basic() {
    let test_app = TestApp::new().await;

    let table_name = "upsert_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let query_a = vec![builders::single_id(20, 0, 100, 100)];
    put_data(&test_app, table_name, num(1.0), query_a.clone()).await;

    let query_b = vec![builders::single_id(19, 0, 50, 50)];
    patch_data(&test_app, table_name, num(10.0), query_b).await;

    let res_a = search_data(&test_app, table_name, query_a).await;
    assert_first_entry(
        &res_a,
        &num(1.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 100,
            y: 100,
            i: None,
            t: None,
        },
    );

    let query_c = vec![builders::single_id(20, 0, 101, 100)];
    let res_c = search_data(&test_app, table_name, query_c).await;
    assert_first_entry(
        &res_c,
        &num(10.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 101,
            y: 100,
            i: None,
            t: None,
        },
    );
}
