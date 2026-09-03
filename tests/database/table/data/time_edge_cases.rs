//! 時間軸の入出力に関する境界値・コーナーケースの検証。
//!
//! - 入力バリデーション（`i`/`t` の組み合わせ、暦の単位以外の拒否、 FlexId のズーム範囲）
//! - 過剰結合・過小結合が起きていないか（隣接していないSegmentや値が違うSegmentは
//!   結合されてはならない／暦のより粗い単位に丸め込めるときはそちらへ丸め込まれる）
//! - 上書き・削除が時間Segment単位で正しく効くか
//! - 境界値でのラウンドトリップ（秒単位・全時間）

use std::collections::BTreeSet;

use kasane::grpc::pb;
use tonic::Status;
use tonic_types::StatusExt;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::{put_data, search_data};

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
        .insert(pb::InsertDataRequest {
            db_name: "test_db".to_string(),
            table_name: "test_table".to_string(),
            value: Some(value),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .map(|_| ())
}

async fn remove_data(test_app: &TestApp, spatial_ids: Vec<pb::SpatialId>) {
    test_app
        .data()
        .remove(pb::RemoveDataRequest {
            db_name: "test_db".to_string(),
            table_name: "test_table".to_string(),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .expect("remove failed");
}

/// `format=rangeId` で検索する。
async fn search_range(
    test_app: &TestApp,
    spatial_ids: Vec<pb::SpatialId>,
) -> pb::SearchDataResponse {
    test_app
        .data()
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: "test_table".to_string(),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::RangeId as i32,
            limit: None,
        })
        .await
        .unwrap()
        .into_inner()
}

/// `format=flexId` で検索する。
async fn search_flex(
    test_app: &TestApp,
    spatial_ids: Vec<pb::SpatialId>,
) -> pb::SearchDataResponse {
    test_app
        .data()
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: "test_table".to_string(),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::FlexId as i32,
            limit: None,
        })
        .await
        .unwrap()
        .into_inner()
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

fn single_id_t_only(z: u32, f: i32, x: u32, y: u32, t: u64) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::SingleId(pb::SingleId {
            z,
            f,
            x,
            y,
            i: None,
            t: Some(t),
        })),
    }
}

fn range_id_i_only(z: u32, f: (i32, i32), x: (u32, u32), y: (u32, u32), i: u64) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::RangeId(pb::RangeId {
            z,
            f: Some(pb::Int32Range { min: f.0, max: f.1 }),
            x: Some(pb::Uint32Range { min: x.0, max: x.1 }),
            y: Some(pb::Uint32Range { min: y.0, max: y.1 }),
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
async fn t_without_i_is_rejected_for_single_id() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(&test_app, num(1.0), vec![single_id_t_only(20, 0, 1, 1, 0)])
        .await
        .expect_err("t without i must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn i_without_t_is_rejected_for_range_id() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![range_id_i_only(20, (0, 0), (1, 1), (1, 1), 3600)],
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

#[tokio::test]
async fn t_index_without_t_zoomlevel_is_rejected_for_flex_id() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![flex_id_raw(20, 0, 20, 1, 20, 1, None, Some(5))],
    )
    .await
    .expect_err("tIndex without tZoomlevel must be rejected");
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
        assert_eq!(error_reason(&err), "invalid_spatial_id", "i={i}");
    }
}

#[tokio::test]
async fn zero_interval_is_rejected() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![builders::single_id_with_time(20, 0, 1, 1, 0, 0)],
    )
    .await
    .expect_err("i=0 must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert_eq!(error_reason(&err), "invalid_spatial_id");
}

/// 「時間指定が不正」というひとつのユーザーミスに、複数のエラーコードを割り当てない。
///
/// `i=0` は kasane-logic の `Interval::new` が、`i=7` はこちらの暦チェックが、
/// `t` の範囲外は `with_time` が弾く——と検出箇所はバラバラだが、クライアントから見れば
/// どれも同じ「`i`/`t` の指定ミス」なので、`code` は1つに揃っていなければならない。
#[tokio::test]
async fn every_invalid_time_specification_shares_one_error_code() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    let cases: Vec<(&str, pb::SpatialId)> = vec![
        // `Interval::new` が弾く（0 / 上限超え）
        ("i=0", builders::single_id_with_time(20, 0, 1, 1, 0, 0)),
        (
            "i over max",
            builders::single_id_with_time(20, 0, 1, 1, 34359738369, 0),
        ),
        // 暦の単位チェックが弾く
        ("i=7", builders::single_id_with_time(20, 0, 1, 1, 7, 0)),
        // `with_time` が弾く（区間の終端が 2^35 秒を超える）
        (
            "t out of range",
            builders::single_id_with_time(20, 0, 1, 1, 86400, u64::MAX),
        ),
        // 片方だけの指定
        ("i without t", single_id_i_only(20, 0, 1, 1, 3600)),
        ("t without i", single_id_t_only(20, 0, 1, 1, 0)),
        // FlexId 側も同じコードに揃える
        (
            "flexId tZoomlevel without tIndex",
            flex_id_raw(20, 0, 20, 1, 20, 1, Some(25), None),
        ),
        (
            "flexId tZoomlevel out of range",
            flex_id_raw(20, 0, 20, 1, 20, 1, Some(36), Some(0)),
        ),
    ];

    for (label, spatial_id) in cases {
        let err = insert_result(&test_app, num(1.0), vec![spatial_id])
            .await
            .expect_err(&format!("{label} was unexpectedly accepted"));
        assert_eq!(err.code(), tonic::Code::InvalidArgument, "{label}: {err:?}");
        assert_eq!(
            error_reason(&err),
            "invalid_spatial_id",
            "{label} should use the same error code as every other bad time spec: {err:?}"
        );
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

/// `tZoomlevel=1` はSegmentが2つ（インデックス0,1）しかないので、`tIndex=5` は範囲外。
#[tokio::test]
async fn flex_id_t_index_out_of_range_is_rejected() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;
    let err = insert_result(
        &test_app,
        num(1.0),
        vec![flex_id_raw(20, 0, 20, 1, 20, 1, Some(1), Some(5))],
    )
    .await
    .expect_err("tIndex out of range must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

// ---------------------------------------------------------------------------
// 過剰結合・過小結合
// ---------------------------------------------------------------------------

/// 同じ値でも、間に隙間がある2つのSegmentは結合されてはならない
/// （t=0とt=5をひとまとめの範囲[0,5]にしてしまうと、挿入していないt=1..4も
/// 含まれることになり、存在しないデータを存在するかのように返してしまう）。
#[tokio::test]
async fn non_adjacent_segments_do_not_coalesce() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    for t in [0, 5] {
        let query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, t)];
        put_data(&test_app, "test_table", num(42.0), query).await;
    }

    let query_all = vec![builders::range_id_with_time(
        20,
        Some((0, 0)),
        Some((1, 1)),
        Some((1, 1)),
        3600,
        (0, 5),
    )];
    let result = search_range(&test_app, query_all).await;
    assert_eq!(
        result.data.len(),
        1,
        "both segments share the same value, so one dictionary entry"
    );
    let group = &result.data[0];
    assert_eq!(
        group.spatial_ids.len(),
        2,
        "gapped segments must stay as two separate entries, got {:?}",
        group.spatial_ids
    );
    let ts: BTreeSet<u64> = group
        .spatial_ids
        .iter()
        .map(|id| match &id.kind {
            Some(pb::spatial_id::Kind::RangeId(r)) => r.t.as_ref().unwrap().min,
            other => panic!("expected a RangeId, got {other:?}"),
        })
        .collect();
    assert_eq!(ts, [0, 5].into_iter().collect());
}

/// 隣接していても値が異なるSegmentは結合されてはならない。
#[tokio::test]
async fn different_values_do_not_coalesce_even_if_adjacent() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    for (t, value) in [(0, 1.0), (1, 2.0)] {
        let query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, t)];
        put_data(&test_app, "test_table", num(value), query).await;
    }

    let query_all = vec![builders::range_id_with_time(
        20,
        Some((0, 0)),
        Some((1, 1)),
        Some((1, 1)),
        3600,
        (0, 1),
    )];
    let result = search_range(&test_app, query_all).await;
    assert_eq!(
        result.data.len(),
        2,
        "different values must stay in separate dictionary groups"
    );
    for group in &result.data {
        assert_eq!(
            group.spatial_ids.len(),
            1,
            "each group should hold exactly its own segment"
        );
        match &group.spatial_ids[0].kind {
            Some(pb::spatial_id::Kind::RangeId(r)) => assert_eq!(r.i, Some(3600)),
            other => panic!("expected a RangeId, got {other:?}"),
        }
    }
}

/// 1時間 (`HOUR`) を24個ぶん連続で挿入すると、読み出し時は暦の中で最も粗い単位
/// （`DAY` = 86400秒）へ丸め込まれ、単一のエントリとして戻る。
#[tokio::test]
async fn twenty_four_adjacent_hours_coalesce_into_a_day() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    for t in 0..24 {
        let query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, t)];
        put_data(&test_app, "test_table", num(42.0), query).await;
    }

    let query_all = vec![builders::range_id_with_time(
        20,
        Some((0, 0)),
        Some((1, 1)),
        Some((1, 1)),
        3600,
        (0, 23),
    )];
    let result = search_range(&test_app, query_all).await;
    assert_eq!(result.data.len(), 1);
    let group = &result.data[0];
    assert_eq!(
        group.spatial_ids.len(),
        1,
        "24 contiguous hours spanning exactly one day should coalesce into a single entry, got {:?}",
        group.spatial_ids
    );
    match &group.spatial_ids[0].kind {
        Some(pb::spatial_id::Kind::RangeId(r)) => {
            assert_eq!(
                r.i,
                Some(86400),
                "should roll up to the coarsest calendar unit that exactly covers the range (DAY), got {r:?}"
            );
            // RangeId の t は常に [min, max] なので、単一点でも [0, 0] になる。
            assert_eq!(r.t, Some(pb::Uint64Range { min: 0, max: 0 }));
        }
        other => panic!("expected a RangeId, got {other:?}"),
    }
}

/// `flexId` フォーマットでは、時間方向の暦への結合は行わない（木の生Segmentをそのまま返す）。
///
/// `i=3600`（時、2の冪ではない）で挿入すると、挿入時点で複数の生Segmentへ分解される。
/// `rangeId`/`singleId` 出力はこれを暦の単位へ結合し直して1件にするが、`flexId` 出力は
/// 結合を行わないため、生Segment数（1件より多い）がそのまま見える。
/// （`i=1` のような2の冪の間隔だと、隣接Segmentは木自体がストレージ時点で1つの物理
/// ノードへ圧縮してしまうため、このテストの意図には使えない。）
#[tokio::test]
async fn flex_id_format_does_not_coalesce_across_segments() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    for t in [0, 1] {
        let query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, t)];
        put_data(&test_app, "test_table", num(42.0), query).await;
    }

    let query_all = vec![builders::range_id_with_time(
        20,
        Some((0, 0)),
        Some((1, 1)),
        Some((1, 1)),
        3600,
        (0, 1),
    )];

    // rangeId: 暦の単位へ結合され1件になる。
    let range_result = search_range(&test_app, query_all.clone()).await;
    assert_eq!(range_result.data.len(), 1);
    let range_group = &range_result.data[0];
    assert_eq!(
        range_group.spatial_ids.len(),
        1,
        "rangeId output should coalesce the two adjacent hours into one entry, got {:?}",
        range_group.spatial_ids
    );
    match &range_group.spatial_ids[0].kind {
        Some(pb::spatial_id::Kind::RangeId(r)) => {
            assert_eq!(r.i, Some(3600));
            assert_eq!(r.t, Some(pb::Uint64Range { min: 0, max: 1 }));
        }
        other => panic!("expected a RangeId, got {other:?}"),
    }

    // flexId: 生Segmentのままなので、rangeId側より多いエントリ数になる（結合しない）。
    let flex_result = search_flex(&test_app, query_all).await;
    assert_eq!(flex_result.data.len(), 1);
    let flex_group = &flex_result.data[0];
    assert!(
        flex_group.spatial_ids.len() > 1,
        "flexId output must not be coalesced down to a single entry like rangeId is, got {:?}",
        flex_group.spatial_ids
    );
}

// ---------------------------------------------------------------------------
// 上書き・削除
// ---------------------------------------------------------------------------

/// 同じ時間Segmentへの上書きは、新しい値だけが残る（古い値が残留しない）。
#[tokio::test]
async fn overwrite_replaces_the_value_at_the_same_time_segment() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    let query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, 0)];
    put_data(&test_app, "test_table", num(1.0), query.clone()).await;
    put_data(&test_app, "test_table", num(2.0), query.clone()).await;

    let result = search_data(&test_app, "test_table", query).await;
    assert_eq!(
        result.data.len(),
        1,
        "only the new value should remain, got {:?}",
        result.data
    );
    let group = &result.data[0];
    let value = result.dictionary.get(group.value_ref as usize).unwrap();
    assert_eq!(value, &num(2.0));
}

/// 隣接する時間Segmentの片方を削除しても、もう片方には影響しない。
#[tokio::test]
async fn removing_one_segment_leaves_the_adjacent_segment_intact() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    for t in [0, 1] {
        let query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, t)];
        put_data(&test_app, "test_table", num(42.0), query).await;
    }

    remove_data(
        &test_app,
        vec![builders::single_id_with_time(20, 0, 1, 1, 3600, 0)],
    )
    .await;

    let remaining_query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, 1)];
    let result = search_data(&test_app, "test_table", remaining_query).await;
    assert_eq!(
        result.data.len(),
        1,
        "t=1 must still be present after removing t=0"
    );

    let removed_query = vec![builders::single_id_with_time(20, 0, 1, 1, 3600, 0)];
    let result = search_data(&test_app, "test_table", removed_query).await;
    assert!(
        result.data.is_empty(),
        "t=0 must be gone after removal, got {:?}",
        result.data
    );
}

// ---------------------------------------------------------------------------
// ラウンドトリップの境界値
// ---------------------------------------------------------------------------

/// `i=1`（秒、2の冪）は分解が起きないため、常に厳密にラウンドトリップする。
#[tokio::test]
async fn second_granularity_round_trips_exactly() {
    let test_app = TestApp::new().await;
    setup(&test_app).await;

    let query = vec![builders::single_id_with_time(20, 0, 1, 1, 1, 1_700_000_000)];
    put_data(&test_app, "test_table", num(42.0), query.clone()).await;

    let result = search_data(&test_app, "test_table", query).await;
    assert_eq!(result.data.len(), 1);
    let group = &result.data[0];
    assert_eq!(group.spatial_ids.len(), 1);
    match &group.spatial_ids[0].kind {
        Some(pb::spatial_id::Kind::SingleId(s)) => {
            assert_eq!(s.i, Some(1));
            assert_eq!(s.t, Some(1_700_000_000));
        }
        other => panic!("expected a SingleId, got {other:?}"),
    }
}

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
