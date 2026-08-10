//! シャード木の**範囲ルーティング**の検証。
//!
//! 空間IDを個別に辿る `route_leaves_batched`（search が使う）と、範囲で辿る
//! `route_leaves_for_range`（query が使う）が同じリーフ集合へ到達することを確かめる。

use kasane::db_init::initialize_database;
use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::database::table::data::shard;
use kasane::repositories::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{RangeId, SingleId, SpatialIdSet};

#[test]
fn range_routing_reaches_the_same_leaves_as_id_routing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());

    // 4セルを個別のトランザクションで書き込む（API 経由と同じ形）。
    for i in 0..4u32 {
        let wtxn = db.env.write_txn().unwrap();
        let mut w = KasaneDbWrite::new(wtxn, &db);
        let mut set = SpatialIdSet::new();
        set.insert(SingleId::new(20, 0, 790000 + i, 500000).unwrap());
        w.data_insert_impl(table_id, TableDataType::Int, set, &(i as i32).to_be_bytes())
            .unwrap();
        w.commit().unwrap();
    }

    let rtxn = db.env.read_txn().unwrap();
    let r = KasaneDbRead::new(rtxn, &db);

    // 個別ID経由（search と同じ経路）
    let ids: Vec<kasane_logic::FlexId> = (0..4)
        .map(|i| {
            let mut s = SpatialIdSet::new();
            s.insert(SingleId::new(20, 0, 790000 + i, 500000).unwrap());
            s.iter().next().unwrap()
        })
        .collect();
    let by_id =
        shard::route_leaves_batched(&db.tables_data, &r.read_txn, table_id, ids.iter()).unwrap();
    let mut id_leaves: Vec<_> = by_id.keys().cloned().collect();
    id_leaves.sort();

    // 範囲経由（query と同じ経路）
    let bbox = RangeId::new(20, [0, 0], [790000, 790003], [500000, 500000]).unwrap();
    let mut range_leaves =
        shard::route_leaves_for_range(&db.tables_data, &r.read_txn, table_id, &bbox).unwrap();
    range_leaves.sort();

    assert_eq!(
        range_leaves, id_leaves,
        "範囲ルーティングが個別IDルーティングと同じリーフに到達していない"
    );
}

/// `TableSource::read_subset` が範囲内の全セルを返すこと。
#[test]
fn table_source_read_subset_returns_all_cells() {
    use kasane::repositories::database::table::data::query_source::TableSource;
    use kasane_logic::Source;
    use std::sync::Arc;

    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());

    for i in 0..4u32 {
        let wtxn = db.env.write_txn().unwrap();
        let mut w = KasaneDbWrite::new(wtxn, &db);
        let mut set = SpatialIdSet::new();
        set.insert(SingleId::new(20, 0, 790000 + i, 500000).unwrap());
        w.data_insert_impl(table_id, TableDataType::Int, set, &(i as i32).to_be_bytes())
            .unwrap();
        w.commit().unwrap();
    }

    let source: TableSource<i32> = TableSource::new(
        db.env.clone(),
        db.tables_data,
        table_id,
        Arc::new(|b: &[u8]| <[u8; 4]>::try_from(b).ok().map(i32::from_be_bytes)),
    );

    let bbox = RangeId::new(20, [0, 0], [790000, 790003], [500000, 500000]).unwrap();
    let working = source.read_subset(&[bbox]).unwrap();
    let mut got: Vec<i32> = working.into_iter().map(|(_, v)| v).collect();
    got.sort();

    assert_eq!(got, vec![0, 1, 2, 3], "read_subset が取りこぼしている");
}

/// 対象空間IDの集合から得た外接範囲が、全セルを覆っていること。
#[test]
fn bounding_box_of_target_set_covers_every_cell() {
    let mut set = SpatialIdSet::new();
    for i in 0..4u32 {
        set.insert(SingleId::new(20, 0, 790000 + i, 500000).unwrap());
    }

    let bbox = set.bounding_box().expect("空でない");
    assert_eq!(bbox.z(), 20, "bbox={bbox}");
    assert_eq!(bbox.x(), [790000, 790003], "bbox={bbox}");
    assert_eq!(bbox.y(), [500000, 500000], "bbox={bbox}");

    // 各セルが集合に含まれると判定されること（最終フィルタと同じ条件）。
    for i in 0..4u32 {
        let id = SingleId::new(20, 0, 790000 + i, 500000).unwrap();
        assert!(
            set.get(&id).next().is_some(),
            "x={} が集合に含まれない",
            790000 + i
        );
    }
}
