use kasane::grpc::pb;

use crate::common::TestApp;

#[tokio::test]
/// テーブル情報が正しく取得できることを検証する。
async fn test_table_info_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .create_table("test_db", "info_target_table", "Int", 15)
        .await;

    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "info_target_table".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(info.name, "info_target_table");
    assert_eq!(info.data_type, pb::TableDataType::Int as i32);
    assert_eq!(info.max_zoom_level, 15);
}

#[tokio::test]
/// 存在しないテーブルの情報取得リクエストが404エラーとなることを検証する。
async fn test_table_info_not_found() {
    let test_app = TestApp::new().await;

    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "example_table", "Int", 25)
        .await;

    let result = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "non_existent_table".to_string(),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}
