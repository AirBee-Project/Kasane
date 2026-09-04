//! データベース単位の CRUD（作成・一覧・改名・複製・説明文・削除）。

use kasane::grpc::pb;

use crate::common::TestApp;

#[tokio::test]
/// データベースの作成と一覧取得が正常に行えるかを検証する。
async fn test_create_and_list_database() {
    let test_app = TestApp::new().await;

    test_app
        .database()
        .list(pb::ListDatabasesRequest {})
        .await
        .unwrap();

    test_app
        .database()
        .create(pb::CreateDatabaseRequest {
            name: "test_db".to_string(),
            description: None,
        })
        .await
        .unwrap();

    let list = test_app
        .database()
        .list(pb::ListDatabasesRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(list.databases[0].name, "test_db");
}

#[tokio::test]
/// データベースおよび配下のテーブルが正しく削除されるかを検証する。
async fn test_remove_database() {
    let test_app = TestApp::new().await;

    test_app
        .database()
        .create(pb::CreateDatabaseRequest {
            name: "test_db".to_string(),
            description: None,
        })
        .await
        .unwrap();

    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "test_table".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 25,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .unwrap();

    test_app
        .database()
        .delete(pb::DeleteDatabaseRequest {
            db_name: "test_db".to_string(),
        })
        .await
        .unwrap();

    let result = test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "test_db".to_string(),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
/// データベースの名前変更が正常に行えるかを検証する。
async fn test_database_rename_success() {
    let test_app = TestApp::new().await;

    test_app
        .database()
        .create(pb::CreateDatabaseRequest {
            name: "test_db".to_string(),
            description: None,
        })
        .await
        .unwrap();

    test_app
        .database()
        .update(pb::UpdateDatabaseRequest {
            db_name: "test_db".to_string(),
            new_name: Some("renamed_db".to_string()),
            description_update: None,
        })
        .await
        .unwrap();

    test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "renamed_db".to_string(),
        })
        .await
        .unwrap();

    let result = test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "test_db".to_string(),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
/// データベースのコピーが正常に行えるかを検証する。
async fn test_database_copy_success() {
    let test_app = TestApp::new().await;

    test_app
        .database()
        .create(pb::CreateDatabaseRequest {
            name: "src_db".to_string(),
            description: None,
        })
        .await
        .unwrap();

    test_app
        .database()
        .copy(pb::CopyDatabaseRequest {
            db_name: "src_db".to_string(),
            copy_name: "copied_db".to_string(),
        })
        .await
        .unwrap();

    test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "copied_db".to_string(),
        })
        .await
        .unwrap();
}

#[tokio::test]
/// データベースのdescription付与と更新が正常に行えるかを検証する。
async fn test_database_description() {
    let test_app = TestApp::new().await;

    test_app
        .database()
        .create(pb::CreateDatabaseRequest {
            name: "desc_db".to_string(),
            description: Some("This is a test database.".to_string()),
        })
        .await
        .unwrap();

    let info = test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "desc_db".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        info.description.as_deref(),
        Some("This is a test database.")
    );

    test_app
        .database()
        .update(pb::UpdateDatabaseRequest {
            db_name: "desc_db".to_string(),
            new_name: None,
            description_update: Some(
                pb::update_database_request::DescriptionUpdate::SetDescription(
                    "Updated description.".to_string(),
                ),
            ),
        })
        .await
        .unwrap();

    let info = test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "desc_db".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.description.as_deref(), Some("Updated description."));

    test_app
        .database()
        .update(pb::UpdateDatabaseRequest {
            db_name: "desc_db".to_string(),
            new_name: None,
            description_update: Some(
                pb::update_database_request::DescriptionUpdate::ClearDescription(true),
            ),
        })
        .await
        .unwrap();

    let info = test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "desc_db".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.description, None);

    test_app
        .database()
        .update(pb::UpdateDatabaseRequest {
            db_name: "desc_db".to_string(),
            new_name: None,
            description_update: Some(
                pb::update_database_request::DescriptionUpdate::SetDescription(
                    "Temp desc".to_string(),
                ),
            ),
        })
        .await
        .unwrap();

    test_app
        .database()
        .update(pb::UpdateDatabaseRequest {
            db_name: "desc_db".to_string(),
            new_name: Some("desc_db_renamed".to_string()),
            description_update: None,
        })
        .await
        .unwrap();

    let info = test_app
        .database()
        .get(pb::GetDatabaseRequest {
            db_name: "desc_db_renamed".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.description.as_deref(), Some("Temp desc"));
}

#[tokio::test]
/// データベースのdescriptionが4096文字を超える場合にエラーになるかを検証する。
async fn test_database_description_too_long() {
    let test_app = TestApp::new().await;

    let long_desc = "a".repeat(kasane::models::database::MAX_DESCRIPTION_LENGTH + 1);

    let result = test_app
        .database()
        .create(pb::CreateDatabaseRequest {
            name: "desc_db_too_long".to_string(),
            description: Some(long_desc),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}
