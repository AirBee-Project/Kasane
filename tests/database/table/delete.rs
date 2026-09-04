use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{num, single_id};
use crate::common::data::put_data;

#[tokio::test]
/// テーブルが正常に削除され、再取得できないことを検証する。
async fn test_delete_table_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .create_table("test_db", "table_to_delete", "Int", 25)
        .await;

    test_app
        .table()
        .delete(pb::DeleteTableRequest {
            db_name: "test_db".to_string(),
            table_name: "table_to_delete".to_string(),
        })
        .await
        .unwrap();

    let result = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "table_to_delete".to_string(),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
/// 存在しないテーブルの削除リクエストが404エラーとなることを検証する。
async fn test_delete_table_not_found() {
    let test_app = TestApp::new().await;

    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "example_table", "Int", 25)
        .await;

    let result = test_app
        .table()
        .delete(pb::DeleteTableRequest {
            db_name: "test_db".to_string(),
            table_name: "non_existent_table".to_string(),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
/// データが存在するテーブルを削除した後、同名で再作成できること（キャッシュクリア）を検証する。
async fn test_delete_table_cache_bug() {
    let test_app = TestApp::new().await;

    let table_name = "bug1_table";

    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    put_data(
        &test_app,
        table_name,
        num(1.0),
        vec![single_id(20, 0, 931386, 412905)],
    )
    .await;

    test_app
        .table()
        .delete(pb::DeleteTableRequest {
            db_name: "test_db".to_string(),
            table_name: table_name.to_string(),
        })
        .await
        .unwrap();

    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: table_name.to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 25,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .expect("Table should be recreatable after deletion");
}
