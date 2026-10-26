//! 時間軸の入出力の検証。
//!
//! - 入力バリデーション（`i`/`t` の組み合わせ、暦の単位以外の拒否、 FlexId のズーム範囲）
//! - 出力への変換（全時間、時間範囲を持つ RangeId）

use kasane::grpc::pb;
use tonic::Status;
use tonic_types::StatusExt;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::{collect_search_stream, put_data, search_data};

async fn setup(test_app: &TestApp) {
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;
}

fn error_reason(status: &Status) -> String {
    status
        .get_error_details()
        .error_info()
        .map(|info| info.reason.clone())
        .unwrap_or_default()
}

/// `Insert` を投げて結果だけ返す（成功・失敗どちらも許容する）。
async fn insert_result(
    test_app: &TestApp,
    value: pb::TypedValue,
    spatial_ids: Vec<pb::SpatialId>,
) -> Result<(), Status> {
    test_app
        .data()
        .insert(pb::WriteDataRequest {
            db_name: "test_db".to_string(),
            table_name: "test_table".to_string(),
            value: Some(value),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .map(|_| ())
}

/// `format=rangeId` で検索する。
async fn search_range(
    test_app: &TestApp,
    spatial_ids: Vec<pb::SpatialId>,
) -> pb::SearchDataResponse {
    let stream = test_app
        .data()
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: "test_table".to_string(),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::RangeId as i32,
        })
        .await
        .unwrap()
        .into_inner();
    collect_search_stream(stream).await
}

fn single_id_i_only(z: u32, f: i32, x: u32, y: u32, i: u64) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::SingleId(pb::SingleId {
            z,
            f,
            x,
            y,
            i: Some(i),
            t: None,
        })),
    }
}

fn range_id_t_only(
    z: u32,
    f: (i32, i32),
    x: (u32, u32),
    y: (u32, u32),
    t: (u64, u64),
) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::RangeId(pb::RangeId {
            z,
            f: Some(pb::Int32Range { min: f.0, max: f.1 }),
            x: Some(pb::Uint32Range { min: x.0, max: x.1 }),
            y: Some(pb::Uint32Range { min: y.0, max: y.1 }),
            i: None,
            t: Some(pb::Uint64Range { min: t.0, max: t.1 }),
        })),
    }
}

#[allow(clippy::too_many_arguments)]
fn flex_id_raw(
    f_zoomlevel: u32,
    f_index: i32,
    x_zoomlevel: u32,
    x_index: u32,
    y_zoomlevel: u32,
    y_index: u32,
    t_zoomlevel: Option<u32>,
    t_index: Option<u64>,
) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::FlexId(pb::FlexId {
            f_zoomlevel,
            f_index,
            x_zoomlevel,
            x_index,
            y_zoomlevel,
            y_index,
            t_zoomlevel,
            t_index,
        })),
    }
}

// ---------------------------------------------------------------------------
// 入力バリデーション
// ---------------------------------------------------------------------------

#[tokio::test]
async fn i_without_t_is_rejected_for_single_id() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![single_id_i_only(20, 0, 1, 1, 3600)],
    )
    .await
    .expect_err("i without t must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn t_without_i_is_rejected_for_range_id() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![range_id_t_only(20, (0, 0), (1, 1), (1, 1), (0, 1))],
    )
    .await
    .expect_err("t without i must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn t_zoomlevel_without_t_index_is_rejected_for_flex_id() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![flex_id_raw(20, 0, 20, 1, 20, 1, Some(20), None)],
    )
    .await
    .expect_err("tZoomlevel without tIndex must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

/// `i` は暦の単位（1/60/3600/86400/2^35）以外は拒否される。
#[tokio::test]
async fn non_calendar_intervals_are_rejected() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    for i in [2u64, 30, 1800, 7200, 43200, 172800] {
        let err = insert_result(
            &test_app,
            num(1.0),
            vec![builders::single_id_with_time(20, 0, 1, 1, i, 0)],
        )
        .await
        .expect_err(&format!("i={i} should have been rejected"));
        assert_eq!(err.code(), tonic::Code::InvalidArgument, "i={i}");
        assert_eq!(error_reason(&err), "invalid_argument", "i={i}");
    }
}

/// 暦の単位はすべて受理される（境界値を1つずつ確認）。
#[tokio::test]
async fn every_calendar_interval_is_accepted() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    for i in [1u64, 60, 3600, 86400, 34359738368] {
        insert_result(
            &test_app,
            num(1.0),
            vec![builders::single_id_with_time(20, 0, 1, 1, i, 0)],
        )
        .await
        .unwrap_or_else(|e| panic!("i={i} should have been accepted: {e:?}"));
    }
}

#[tokio::test]
async fn flex_id_t_zoomlevel_out_of_range_is_rejected() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![flex_id_raw(20, 0, 20, 1, 20, 1, Some(36), Some(0))],
    )
    .await
    .expect_err("tZoomlevel out of range must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

// ---------------------------------------------------------------------------
// 過剰結合・過小結合
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 上書き・削除
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// ラウンドトリップの境界値
// ---------------------------------------------------------------------------

/// 時間を指定しない場合は「全時間」を表し、出力にも `i`/`t` は現れない。
#[tokio::test]
async fn whole_time_has_no_i_or_t_in_output() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    let query = vec![builders::single_id(20, 0, 1, 1)];
    put_data(&test_app, "test_table", num(42.0), query.clone()).await;

    let result = search_data(&test_app, "test_table", query).await;
    assert_eq!(result.data.len(), 1);
    let group = &result.data[0];
    assert_eq!(group.spatial_ids.len(), 1);
    match &group.spatial_ids[0].kind {
        Some(pb::spatial_id::Kind::SingleId(s)) => {
            assert!(s.i.is_none(), "i must be absent for whole time, got {s:?}");
            assert!(s.t.is_none(), "t must be absent for whole time, got {s:?}");
        }
        other => panic!("expected a SingleId, got {other:?}"),
    }
}

/// `RangeId` として時間の範囲を直接指定して挿入した場合も正しくラウンドトリップする。
#[tokio::test]
async fn range_id_with_a_time_range_round_trips() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    let query = vec![builders::range_id_with_time(
        20,
        Some((0, 0)),
        Some((1, 1)),
        Some((1, 1)),
        3600,
        (0, 1),
    )];
    put_data(&test_app, "test_table", num(42.0), query.clone()).await;

    let result = search_range(&test_app, query).await;
    assert_eq!(result.data.len(), 1);
    let group = &result.data[0];
    assert_eq!(
        group.spatial_ids.len(),
        1,
        "a single RangeId insert covering [0,1] must read back as one entry, got {:?}",
        group.spatial_ids
    );
    match &group.spatial_ids[0].kind {
        Some(pb::spatial_id::Kind::RangeId(r)) => {
            assert_eq!(r.i, Some(3600));
            assert_eq!(r.t, Some(pb::Uint64Range { min: 0, max: 1 }));
        }
        other => panic!("expected a RangeId, got {other:?}"),
    }
}
