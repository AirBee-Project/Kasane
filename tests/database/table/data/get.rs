use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::{collect_search_stream, put_data, search_data, to_result_map};

#[tokio::test]
/// 複数の空間IDを一度に指定してデータを検索・取得できることを検証する。
async fn test_table_data_get_multiple() {
    let test_app = TestApp::new().await;

    let table_name = "get_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let id1 = (20, 0, 10, 10);
    let id2 = (20, 0, 20, 20);

    put_data(
        &test_app,
        table_name,
        num(100.0),
        vec![builders::single_id(20, 0, 10, 10)],
    )
    .await;

    put_data(
        &test_app,
        table_name,
        num(200.0),
        vec![builders::single_id(20, 0, 20, 20)],
    )
    .await;

    let query = vec![
        builders::single_id(20, 0, 10, 10),
        builders::single_id(20, 0, 20, 20),
    ];

    let result = search_data(&test_app, table_name, query).await;
    let result_map = to_result_map::<i64>(&result);

    assert_eq!(result_map.len(), 2);
    assert_eq!(result_map[&id1], 100);
    assert_eq!(result_map[&id2], 200);
}

#[tokio::test]
/// RangeIdと FlexId でのレスポンスフォーマットを検証する。
async fn test_table_data_get_format_options() {
    let test_app = TestApp::new().await;

    let table_name = "get_table_formats";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    put_data(
        &test_app,
        table_name,
        num(500.0),
        vec![builders::single_id(20, 0, 10, 10)],
    )
    .await;

    let query = vec![builders::single_id(20, 0, 10, 10)];

    // Test RangeId
    let res_range = collect_search_stream(
        test_app
            .data()
            .search(pb::SearchDataRequest {
                db_name: "test_db".to_string(),
                table_name: table_name.to_string(),
                spatial_ids: query.clone(),
                zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
                format: pb::OutputFormat::RangeId as i32,
                limit: None,
            })
            .await
            .unwrap()
            .into_inner(),
    )
    .await;

    let group = res_range.data.first().unwrap();
    let first_id = group.spatial_ids.first().unwrap();
    match &first_id.kind {
        Some(pb::spatial_id::Kind::RangeId(_)) => {}
        other => panic!("expected a RangeId, got {other:?}"),
    }

    // Test FlexId
    let res_flex = collect_search_stream(
        test_app
            .data()
            .search(pb::SearchDataRequest {
                db_name: "test_db".to_string(),
                table_name: table_name.to_string(),
                spatial_ids: query,
                zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
                format: pb::OutputFormat::FlexId as i32,
                limit: None,
            })
            .await
            .unwrap()
            .into_inner(),
    )
    .await;

    let group_flex = res_flex.data.first().unwrap();
    let first_id_flex = group_flex.spatial_ids.first().unwrap();
    match &first_id_flex.kind {
        Some(pb::spatial_id::Kind::FlexId(_)) => {}
        other => panic!("expected a FlexId, got {other:?}"),
    }
}
