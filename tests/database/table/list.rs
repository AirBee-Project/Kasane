use kasane::grpc::pb;

use crate::common::TestApp;

#[tokio::test]
/// 初期状態で空のテーブル一覧が取得できることを検証する。
async fn test_table_list_empty() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let list = test_app
        .table()
        .list(pb::ListTablesRequest {
            db_name: "test_db".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(list.tables.len(), 0);
}

#[tokio::test]
/// 2つのテーブルを追加し、一覧に両方が含まれていることを検証する。
async fn test_table_list_two() {
    let test_app = TestApp::new().await;

    test_app.create_database("test_db").await;
    test_app.create_table("test_db", "table_a", "Int", 10).await;
    test_app.create_table("test_db", "table_b", "Int", 20).await;

    let list = test_app
        .table()
        .list(pb::ListTablesRequest {
            db_name: "test_db".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(list.tables.len(), 2);
    let names: Vec<&str> = list.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"table_a"));
    assert!(names.contains(&"table_b"));
}

#[tokio::test]
/// 3つのテーブルを追加し、一覧にすべてが含まれていることを検証する。
async fn test_table_list_three() {
    let test_app = TestApp::new().await;

    test_app.create_database("test_db").await;
    test_app.create_table("test_db", "table_a", "Int", 10).await;
    test_app.create_table("test_db", "table_b", "Int", 20).await;
    test_app
        .create_table("test_db", "table_c", "Text", 25)
        .await;

    let list = test_app
        .table()
        .list(pb::ListTablesRequest {
            db_name: "test_db".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(list.tables.len(), 3);
    let names: Vec<&str> = list.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"table_a"));
    assert!(names.contains(&"table_b"));
    assert!(names.contains(&"table_c"));
}

#[tokio::test]
/// db_nameが空文字列の場合に内部エラーにならず、NotFoundになることを検証する。
async fn test_table_list_empty_db_name() {
    let test_app = TestApp::new().await;

    let result = test_app
        .table()
        .list(pb::ListTablesRequest {
            db_name: String::new(),
        })
        .await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}
