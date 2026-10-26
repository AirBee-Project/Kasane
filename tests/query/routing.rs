use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::lmdb::KasaneDbWrite;
use kasane::repositories::lmdb::initialize_database;
use spatial_id::{RangeId, SingleId, collection::SpatialIdSet, query::CancellationToken};

/// `TableSource::read_flex_ids` が範囲内の全 FlexId を返すこと。
#[test]
fn table_source_read_flex_ids_returns_all_flex_ids() {
    use kasane::repositories::lmdb::query_source::TableSource;
    use spatial_id::query::Source;
    use std::sync::Arc;

    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap()).unwrap();
    let table_id = TableId(uuid::Uuid::now_v7());

    for i in 0..4u32 {
        let wtxn = db.env.write_txn().unwrap();
        let mut w = KasaneDbWrite::new(wtxn, &db);
        let mut set = SpatialIdSet::new();
        set.insert(SingleId::new(20, 0, 790000 + i, 500000).unwrap());
        w.data_insert_impl(
            table_id,
            Some(TableDataType::Int),
            set,
            &(i as i32).to_be_bytes(),
        )
        .unwrap();
        w.commit().unwrap();
    }

    // クエリ 1 回分の断面。書き込みを終えてから開くので、全 FlexId が見えるはず。
    let snapshot = Arc::new(std::sync::Mutex::new(
        db.env.clone().static_read_txn().unwrap(),
    ));
    let source: TableSource<i32> = TableSource::new(
        snapshot,
        db.tables_data,
        table_id,
        Arc::new(|b: &[u8]| <[u8; 4]>::try_from(b).ok().map(i32::from_be_bytes)),
    );

    let bbox = RangeId::new(20, [0, 0], [790000, 790003], [500000, 500000]).unwrap();
    let working = source
        .read_flex_ids(&[bbox], &CancellationToken::new())
        .unwrap();
    let mut got: Vec<i32> = working.map(|r| r.unwrap().1).collect();
    got.sort();

    assert_eq!(got, vec![0, 1, 2, 3], "read_flex_ids が取りこぼしている");
}
