//! 動的シャード分割のエンドツーエンド検証。
//!
//! 閾値 `MAX_FLEX_ID_PER_SHARD`(=4096) を超える数の互いに素なセルを挿入し、
//! 1) 実際に分割が起きてポインタノードが生成されること、
//! 2) 分割後も全件が読み戻せること、
//! 3) `table_count` が全件と一致すること、を直接確認する。

use std::collections::HashSet;

use kasane::db_init::initialize_database;
use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{IterSingleIds, RangeId, SingleId, SpatialIdSet};

#[test]
#[ignore]
fn dynamic_shard_splits_and_reads_back() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());

    // 4096 を超える 5000 個の互いに素なセルを上半球(f=0)へ挿入する。
    // x を 4 間隔にして兄弟マージを防ぎ、各セルが独立リーフになるようにする。
    let n: u32 = 5000;
    {
        let wtxn = db.env.write_txn().unwrap();
        let mut w = KasaneDbWrite::new(wtxn, &db);
        let mut ids = SpatialIdSet::new();
        for i in 0..n {
            ids.insert(SingleId::new(20, 0, i * 4, 0).unwrap());
        }
        w.data_insert(table_id, TableDataType::Text, ids, b"v")
            .unwrap();
        w.commit().unwrap();
    }

    // 1. 動的分割が起きたことを直接検証：このテーブルのシャードキーが複数あり、
    //    ポインタノード（ShardEntry のタグ先頭バイト = 1）が存在する。
    {
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
        assert!(
            key_count > 1,
            "expected split into multiple shards, got {key_count} keys"
        );
        assert!(has_pointer, "expected a pointer node after split");
    }

    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);

    // 2. table_count が全件と一致（ポインタノードは 0 扱い、子リーフ合算）。
    assert_eq!(r.table_count(table_id).unwrap(), n as u64);

    // 3. 全域を range クエリして、分割後も全 5000 セルが読めることを検証。
    let mut query = SpatialIdSet::new();
    query.insert(RangeId::new(20, [0, 0], [0, (n - 1) * 4], [0, 0]).unwrap());
    let got = r.data_get(table_id, query).unwrap();

    let mut xs: HashSet<u32> = HashSet::new();
    for (value, flex_ids) in got {
        assert_eq!(value, b"v".to_vec());
        for flex_id in flex_ids {
            for sid in flex_id.iter_single_ids() {
                xs.insert(sid.x());
            }
        }
    }
    let expected: HashSet<u32> = (0..n).map(|i| i * 4).collect();
    assert_eq!(
        xs, expected,
        "all inserted cells must be readable after split"
    );
}
