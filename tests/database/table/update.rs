use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{num, single_id, single_id_with_time};
use crate::common::data::put_data;

#[tokio::test]
/// テーブルの名前を正常に変更できるかを検証する。
async fn test_update_table_name_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "old_name", "Int", 25)
        .await;

    test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "old_name".to_string(),
            new_name: Some("new_name".to_string()),
            constraints_update: None,
            description_update: None,
            is_temporal: None,
        })
        .await
        .unwrap();

    test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "new_name".to_string(),
        })
        .await
        .unwrap();

    let result = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "old_name".to_string(),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
/// テーブルの制約を正常に追加できるかを検証する。
async fn test_update_table_constraints_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "constrained_table", "Int", 25)
        .await;

    test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "constrained_table".to_string(),
            new_name: None,
            constraints_update: Some(
                pb::update_table_request::ConstraintsUpdate::SetConstraints(
                    pb::UpdateTableConstraints {
                        kind: Some(pb::update_table_constraints::Kind::Int(
                            pb::update_table_constraints::IntUpdate {
                                min_update: Some(
                                    pb::update_table_constraints::int_update::MinUpdate::SetMin(10),
                                ),
                                max_update: Some(
                                    pb::update_table_constraints::int_update::MaxUpdate::SetMax(
                                        100,
                                    ),
                                ),
                            },
                        )),
                    },
                ),
            ),
            description_update: None,
            is_temporal: None,
        })
        .await
        .unwrap();

    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "constrained_table".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    match info.constraints.and_then(|c| c.kind) {
        Some(pb::table_constraints::Kind::Int(int)) => {
            assert_eq!(int.min, Some(10));
            assert_eq!(int.max, Some(100));
        }
        other => panic!("expected Int constraints, got {other:?}"),
    }
}

#[tokio::test]
/// 既存のデータが新しい制約に違反する場合、更新が拒否されることを検証する。
async fn test_update_table_constraints_with_existing_data_violation() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "my_table", "Int", 25)
        .await;

    put_data(
        &test_app,
        "my_table",
        num(5.0),
        vec![single_id(20, 0, 0, 0)],
    )
    .await;

    let result = test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "my_table".to_string(),
            new_name: None,
            constraints_update: Some(
                pb::update_table_request::ConstraintsUpdate::SetConstraints(
                    pb::UpdateTableConstraints {
                        kind: Some(pb::update_table_constraints::Kind::Int(
                            pb::update_table_constraints::IntUpdate {
                                min_update: Some(
                                    pb::update_table_constraints::int_update::MinUpdate::SetMin(10),
                                ),
                                max_update: None,
                            },
                        )),
                    },
                ),
            ),
            description_update: None,
            is_temporal: None,
        })
        .await;
    // データが違反しているため更新が拒否される
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
/// 制約の型がテーブルのデータ型と一致しない場合、拒否されることを検証する。
async fn test_update_table_constraints_type_mismatch() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "my_table", "Int", 25)
        .await;

    let result = test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "my_table".to_string(),
            new_name: None,
            constraints_update: Some(
                pb::update_table_request::ConstraintsUpdate::SetConstraints(
                    pb::UpdateTableConstraints {
                        kind: Some(pb::update_table_constraints::Kind::Text(
                            pb::update_table_constraints::TextUpdate {
                                min_length_update: Some(
                                    pb::update_table_constraints::text_update::MinLengthUpdate::SetMinLength(5),
                                ),
                                max_length_update: None,
                            },
                        )),
                    },
                ),
            ),
            description_update: None,
            is_temporal: None,
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
/// テーブルのdescription更新が正常に行えるかを検証する。
async fn test_update_table_description_success() {
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
            description: Some("Initial description.".to_string()),
            value_index: false,
            is_temporal: true,
        })
        .await
        .unwrap();

    test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "desc_table".to_string(),
            new_name: None,
            constraints_update: None,
            description_update: Some(
                pb::update_table_request::DescriptionUpdate::SetDescription(
                    "Updated description.".to_string(),
                ),
            ),
            is_temporal: None,
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
    assert_eq!(info.description.as_deref(), Some("Updated description."));

    // 削除（ClearDescription 指定）
    test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "desc_table".to_string(),
            new_name: None,
            constraints_update: None,
            description_update: Some(
                pb::update_table_request::DescriptionUpdate::ClearDescription(true),
            ),
            is_temporal: None,
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
    assert_eq!(info.description, None);

    // 再度設定して、nameだけの更新時にdescriptionが維持されるか確認
    test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "desc_table".to_string(),
            new_name: None,
            constraints_update: None,
            description_update: Some(
                pb::update_table_request::DescriptionUpdate::SetDescription(
                    "Temp description".to_string(),
                ),
            ),
            is_temporal: None,
        })
        .await
        .unwrap();

    test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "desc_table".to_string(),
            new_name: Some("desc_table_renamed".to_string()),
            constraints_update: None,
            description_update: None,
            is_temporal: None,
        })
        .await
        .unwrap();

    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "desc_table_renamed".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.description.as_deref(), Some("Temp description"));
}

#[tokio::test]
/// テーブルのdescription更新で4096文字を超える場合にエラーになるかを検証する。
async fn test_update_table_description_too_long() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "desc_table_too_long", "Int", 25)
        .await;

    let long_desc = "a".repeat(kasane::models::database::MAX_DESCRIPTION_LENGTH + 1);

    let result = test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "desc_table_too_long".to_string(),
            new_name: None,
            constraints_update: None,
            description_update: Some(
                pb::update_table_request::DescriptionUpdate::SetDescription(long_desc),
            ),
            is_temporal: None,
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
/// `is_temporal: false` で作成したテーブルは時間付きIDの書き込みが拒否されるが、
/// `is_temporal: true` へ緩めれば通るようになることを検証する。
async fn test_update_table_is_temporal_unlock_allows_temporal_write() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: "locked_table".to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 25,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: false,
        })
        .await
        .unwrap();

    // 一覧・詳細の両方で is_temporal が見えること。
    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "locked_table".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(!info.is_temporal);

    let list = test_app
        .table()
        .list(pb::ListTablesRequest {
            db_name: "test_db".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(!list.tables[0].is_temporal);

    // 時間成分付きの書き込みは拒否される。
    let temporal_id = single_id_with_time(20, 0, 0, 0, 3600, 0);
    let insert = || pb::InsertDataRequest {
        db_name: "test_db".to_string(),
        table_name: "locked_table".to_string(),
        value: Some(num(1.0)),
        spatial_ids: vec![temporal_id],
        zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
    };
    let result = test_app.data().insert(insert()).await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);

    // false への再ロックは拒否される。
    let result = test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "locked_table".to_string(),
            new_name: None,
            constraints_update: None,
            description_update: None,
            is_temporal: Some(false),
        })
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);

    // true への解除は成功し、応答にも反映される。
    test_app
        .table()
        .update(pb::UpdateTableRequest {
            db_name: "test_db".to_string(),
            table_name: "locked_table".to_string(),
            new_name: None,
            constraints_update: None,
            description_update: None,
            is_temporal: Some(true),
        })
        .await
        .unwrap();

    let info = test_app
        .table()
        .get(pb::GetTableRequest {
            db_name: "test_db".to_string(),
            table_name: "locked_table".to_string(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(info.is_temporal);

    // 解除後は同じ時間付きIDの書き込みが通る。
    test_app.data().insert(insert()).await.unwrap();
}
