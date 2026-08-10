//! 【バグ実証】split 後に「分割時に空だった領域」へ挿入すると失われないか検証する。

use kasane::db_init::initialize_database;
use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{SingleId, SpatialIdSet};

// binary covering trie でこのバグ（split 後の空領域への挿入消失）が解消されることの回帰テスト。
#[test]
fn insert_into_empty_region_after_split_is_not_lost() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Text;

    // 1. 5000 セル（y=0,f=0 の x 線）を挿入 → split。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut ids = SpatialIdSet::new();
        for i in 0..5000u32 {
            ids.insert(SingleId::new(20, 0, i * 4, 0).unwrap());
        }
        w.data_insert_impl(table_id, dt, ids, b"v").unwrap();
        w.commit().unwrap();
    }

    // 2. 分割時に空だった「広い」領域（x ≥ 2^19、データは x < 2^19 に集中）へ1セル挿入。
    //    この x-upper 兄弟は split 時に空でスキップされている。
    let target = SingleId::new(20, 0, 600000, 0).unwrap();
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut ids = SpatialIdSet::new();
        ids.insert(target.clone());
        w.data_insert_impl(table_id, dt, ids, b"w").unwrap();
        w.commit().unwrap();
    }

    // 3. それが読み戻せるか？
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
    let mut q = SpatialIdSet::new();
    q.insert(target);
    let got = r.data_get_impl(table_id, q, None).unwrap();
    let total: usize = got.iter().map(|(_, fids)| fids.len()).sum();
    assert_eq!(
        total, 1,
        "insert into a region that was empty at split time was LOST"
    );
}
