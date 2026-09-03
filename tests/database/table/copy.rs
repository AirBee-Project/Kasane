use kasane::grpc::pb;

use crate::common::TestApp;

#[tokio::test]
/// テーブルのコピーが同一データベース内で正常に行えるかを検証する。
async fn test_table_copy_success_same_db() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .create_table("test_db", "src_table", "Int", 25)
        .await;

    test_app
        .table()
        .copy(pb::CopyTableRequest {
            db_name: "test_db".to_string(),
            table_name: "src_table".to_string(),
            copy_db_name: None,
            copy_table_name: "copied_table".to_string(),
        })
        .await
        .unwrap();

    test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "copied_table".to_string(),
        })
        .await
        .unwrap();
}
