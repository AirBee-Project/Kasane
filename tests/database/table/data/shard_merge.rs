//! 兄弟シャードの自動マージ（split の逆）のエンドツーエンド検証。
//!
//! 大量挿入で分割（ポインタノード生成）させた後、大量削除で兄弟合算が
//! 低ウォーターマークを割ると、ポインタノードが畳まれて1リーフへ統合されることを確認する。

use std::collections::HashSet;

use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::lmdb::initialize_database;
use kasane::repositories::lmdb::shard::{MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD};
use kasane::repositories::lmdb::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{RangeId, SingleId, SpatialIdSet};

/// このテーブルのシャードキー数と、ポインタノードの有無を返す。
fn shard_stats(db: &kasane::repositories::lmdb::AppDb, table_id: TableId) -> (usize, bool) {
    let rtxn = db.env.read_txn().unwrap();
    let mut key_count = 0usize;
    let mut has_pointer = false;
    for item in db.tables_data.iter(&rtxn).unwrap() {
        let ((tid, _flex), val) = item.unwrap();
        if tid != table_id {
            continue;
        }
        key_count += 1;
        if val.first() == Some(&1) {
            has_pointer = true;
        }
    }
    (key_count, has_pointer)
}

#[test]
fn siblings_merge_after_mass_remove() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Text;

    // 1. 分割が必ず起きる数を挿入する（閾値の2倍）。
    let n: u32 = (MAX_FLEX_ID_PER_SHARD * 2) as u32;
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut ids = SpatialIdSet::new();
        for i in 0..n {
            ids.insert(SingleId::new(20, 0, i * 4, 0).unwrap());
        }
        w.data_insert_impl(table_id, Some(dt), ids, b"v").unwrap();
        w.commit().unwrap();
    }
    let (key_count, has_pointer) = shard_stats(&db, table_id);
    assert!(
        key_count > 1 && has_pointer,
        "insert should have split into a pointer node"
    );

    // 2. 残りがマージ閾値を下回るまで削除する → 兄弟統合。
    let remaining_target: u32 = (MERGE_FLEX_ID_THRESHOLD / 2) as u32;
    let keep_from: u32 = n - remaining_target;
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut ids = SpatialIdSet::new();
        for i in 0..keep_from {
            ids.insert(SingleId::new(20, 0, i * 4, 0).unwrap());
        }
        w.data_remove_impl(table_id, Some(dt), ids).unwrap();
        w.commit().unwrap();
    }

    // 3. ポインタノードが畳まれて単一リーフへ戻ったことを検証。
    let (key_count, has_pointer) = shard_stats(&db, table_id);
    assert_eq!(
        key_count, 1,
        "siblings should have merged back into a single leaf"
    );
    assert!(!has_pointer, "no pointer node should remain after merge");

    // 4. table_count が残数と一致、かつ残りの全 FlexId が読めること。
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
    let remaining = (n - keep_from) as u64;
    assert_eq!(r.table_count_impl(table_id).unwrap(), remaining);

    let mut query = SpatialIdSet::new();
    query.insert(RangeId::new(20, [0, 0], [keep_from * 4, (n - 1) * 4], [0, 0]).unwrap());
    let got = r.data_get_impl(table_id, query, None).unwrap();
    let mut xs: HashSet<u32> = HashSet::new();
    for (value, flex_ids) in got {
        assert_eq!(value, b"v".to_vec());
        for flex_id in flex_ids {
            for sid in flex_id.single_ids() {
                xs.insert(sid.x());
            }
        }
    }
    let expected: HashSet<u32> = (keep_from..n).map(|i| i * 4).collect();
    assert_eq!(
        xs, expected,
        "remaining flex_ids must be readable after merge"
    );
}
