use kasane::grpc::pb;

use crate::common::TestApp;

#[tokio::test]
/// テーブルの正常な作成と取得を検証する。
async fn test_create_table_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "new_table".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 25,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .unwrap();

    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "new_table".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(info.name, "new_table");
    assert_eq!(info.data_type, pb::TableDataType::Int as i32);
    assert_eq!(info.max_zoom_level, 25);
}

#[tokio::test]
/// 同名テーブルの作成が競合エラーとなり、既存のテーブルが保持されることを検証する。
async fn test_create_table_conflict() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .create_table("test_db", "existing_table", "Int", 25)
        .await;

    let result = test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "existing_table".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 20,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::AlreadyExists);

    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "existing_table".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(info.name, "existing_table");
    assert_eq!(info.data_type, pb::TableDataType::Int as i32);
    assert_eq!(info.max_zoom_level, 25);
}

#[tokio::test]
/// max_zoom_level がシステム上限(30)を超える場合は 400 で拒否され、テーブルは作成されない。
async fn test_create_table_max_zoom_level_too_large() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let result = test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "too_deep".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 31,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);

    // 検証で弾かれているので、テーブルは作成されていない。
    let result = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "too_deep".to_string(),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
/// max_zoom_level の境界値 30（システム上限）は許可される。
async fn test_create_table_max_zoom_level_boundary_ok() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "boundary_table".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 30,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .unwrap();
}

#[tokio::test]
/// ENUM型のテーブル作成時に、選択肢の文字列長さが制限(最大255文字、空文字禁止)に従っているか検証する。
async fn test_create_table_enum_choice_length_limits() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    // 1. 256文字の選択肢（エラーになるべき）
    let long_choice = "a".repeat(256);
    let result = test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "too_long_enum".to_string(),
            data_type: pb::TableDataType::Enum as i32,
            max_zoom_level: 25,
            constraints: Some(pb::TableConstraints {
                kind: Some(pb::table_constraints::Kind::EnumConstraint(
                    pb::table_constraints::Enum {
                        choices: vec![long_choice],
                    },
                )),
            }),
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);

    // 2. 空文字の選択肢（エラーになるべき）
    let result = test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "empty_enum".to_string(),
            data_type: pb::TableDataType::Enum as i32,
            max_zoom_level: 25,
            constraints: Some(pb::TableConstraints {
                kind: Some(pb::table_constraints::Kind::EnumConstraint(
                    pb::table_constraints::Enum {
                        choices: vec![String::new()],
                    },
                )),
            }),
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);

    // 3. 255文字の選択肢（成功するべき）
    let border_choice = "a".repeat(255);
    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "ok_enum".to_string(),
            data_type: pb::TableDataType::Enum as i32,
            max_zoom_level: 25,
            constraints: Some(pb::TableConstraints {
                kind: Some(pb::table_constraints::Kind::EnumConstraint(
                    pb::table_constraints::Enum {
                        choices: vec![border_choice],
                    },
                )),
            }),
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .unwrap();
}

#[tokio::test]
/// テーブルのdescription付与が正常に行えるかを検証する。
async fn test_create_table_description() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "desc_table".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 25,
            constraints: None,
            description: Some("This is a test table.".to_string()),
            value_index: false,
            is_temporal: true,
        })
        .await
        .unwrap();

    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "desc_table".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(info.description.as_deref(), Some("This is a test table."));
}

#[tokio::test]
/// テーブルのdescriptionが4096文字を超える場合にエラーになるかを検証する。
async fn test_create_table_description_too_long() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let long_desc = "a".repeat(kasane::models::database::MAX_DESCRIPTION_LENGTH + 1);

    let result = test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "desc_table_too_long".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 25,
            constraints: None,
            description: Some(long_desc),
            value_index: false,
            is_temporal: true,
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}
