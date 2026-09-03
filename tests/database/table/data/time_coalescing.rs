//! 検索結果の時間表現が、暦の単位（`AllowedIntervals::calendar`）へ正しく
//! 結合し直される（coalesce される）ことを検証する。
//!
//! `i`（時間間隔）は入力時に暦の単位（`1`/`60`/`3600`/`86400`/`34359738368`）のみに
//! 制限される。`86400`（1日）は2の冪ではないため、挿入時は内部で複数の木Segmentへ
//! 分解されるが、読み出し時には暦の単位に結合し直され、単一のエントリとして戻るはず。

use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::{put_data, search_data};

#[tokio::test]
async fn read_back_coalesces_a_non_power_of_two_calendar_interval() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query = vec![builders::single_id_with_time(
        20, 0, 931386, 412905, 86400, 5,
    )];

    put_data(&test_app, "test_table", num(7.0), single_id_query.clone()).await;

    let result = search_data(&test_app, "test_table", single_id_query).await;

    assert_eq!(result.data.len(), 1);
    let group = &result.data[0];
    assert_eq!(
        group.spatial_ids.len(),
        1,
        "expected the day-long segment to coalesce back into a single entry, got {:?}",
        group.spatial_ids
    );

    match &group.spatial_ids[0].kind {
        Some(pb::spatial_id::Kind::SingleId(s)) => {
            assert_eq!(
                s,
                &pb::SingleId {
                    z: 20,
                    f: 0,
                    x: 931386,
                    y: 412905,
                    i: Some(86400),
                    t: Some(5),
                }
            );
        }
        other => panic!("expected a SingleId, got {other:?}"),
    }
}

/// 隣接する2つの `HOUR`（3600秒）Segmentを別々に挿入しても、読み出し時には
/// `i=3600, t=[0,1]` の1エントリへ結合される。
#[tokio::test]
async fn read_back_coalesces_adjacent_segments_into_a_range() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    for t in [0, 1] {
        let query = vec![builders::single_id_with_time(
            20, 0, 931386, 412905, 3600, t,
        )];
        put_data(&test_app, "test_table", num(9.0), query).await;
    }

    let range_query = vec![builders::range_id_with_time(
        20,
        Some((0, 0)),
        Some((931386, 931386)),
        Some((412905, 412905)),
        3600,
        (0, 1),
    )];

    let result = test_app
        .data()
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: "test_table".to_string(),
            spatial_ids: range_query,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::RangeId as i32,
            limit: None,
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(result.data.len(), 1);
    let group = &result.data[0];
    assert_eq!(
        group.spatial_ids.len(),
        1,
        "expected the two adjacent hour segments to coalesce into one range, got {:?}",
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
