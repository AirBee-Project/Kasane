//! バイト数トリガによる動的シャード分割のエンドツーエンド検証。
//!
//! 件数は `MAX_FLEX_ID_PER_SHARD` を大きく下回るが、1 件あたりの値を大きくして
//! 合計バイト数が `MAX_SHARD_BYTES` を超えるようにする。件数だけを見ていた旧実装では
//! 分割されず、1 リーフが際限なく肥大化してしまう。
//!
//! 各値は互いに異なるバイト列にする: 保存形式は値を辞書へ集約して重複排除するため、
//! 全件が同一バイト列だと重複排除で 1 コピーにまとまってしまい、バイト数トリガの検証に
//! ならない。

use std::collections::HashSet;

use kasane::models::id::TableId;
use kasane::repositories::encoding::shard_entry::{MAX_FLEX_ID_PER_SHARD, MAX_SHARD_BYTES};
use kasane::repositories::lmdb::initialize_database;
use kasane::repositories::lmdb::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{RangeId, SingleId, SpatialIdSet};

#[test]
fn shard_splits_on_byte_size_even_with_few_entries() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap()).unwrap();
    let table_id = TableId(uuid::Uuid::now_v7());

    // value_index は使わない: Text 系は索引キーへ値をそのまま埋め込むため、この巨大な
    // テスト値だと LMDB のキー長上限に別の理由で当たってしまい、分割ロジックの検証に
    // ならなくなる。
    let index = None;

    // 合計が MAX_SHARD_BYTES を超えるだけの件数（それぞれは MAX_STORED_VALUE_BYTES 未満）。
    // 互いに素な隣接 FlexId として、分割前は同一リーフに収まるようにする。
    let value_len = 64 * 1024;
    let n: u32 = (MAX_SHARD_BYTES / value_len) as u32 + 2;
    assert!(
        (n as usize) < MAX_FLEX_ID_PER_SHARD,
        "test setup must stay under the entry-count threshold, otherwise it no longer isolates byte-size splitting"
    );
    let value_for = |i: u32| -> Vec<u8> {
        let mut v = vec![b'v'; value_len];
        v[..4].copy_from_slice(&i.to_le_bytes());
        v
    };

    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        for i in 0..n {
            let mut ids = SpatialIdSet::new();
            ids.insert(SingleId::new(20, 0, i, 0).unwrap());
            w.data_insert_impl(table_id, index, ids, &value_for(i))
                .unwrap_or_else(|e| panic!("insert {i} failed: {e:?}"));
        }
        w.commit().unwrap();
    }

    // 1. 件数は閾値未満なのに分割が起きている(=バイト数トリガ)ことを直接検証。
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
    drop(rtxn);
    assert!(
        key_count > 1 && has_pointer,
        "expected a byte-size-triggered split with only {n} entries, got {key_count} shard keys"
    );

    // 2. 分割後も全件が読み戻せること。
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
    assert_eq!(r.table_count_impl(table_id).unwrap(), n as u64);

    let mut query = SpatialIdSet::new();
    query.insert(RangeId::new(20, [0, 0], [0, n - 1], [0, 0]).unwrap());
    let got = r.data_get_impl(table_id, query, None).unwrap();

    let mut xs: HashSet<u32> = HashSet::new();
    for (got_value, flex_ids) in got {
        for flex_id in flex_ids {
            for sid in flex_id.single_ids() {
                assert_eq!(
                    got_value,
                    value_for(sid.x()),
                    "value mismatch at x={}",
                    sid.x()
                );
                xs.insert(sid.x());
            }
        }
    }
    let expected: HashSet<u32> = (0..n).collect();
    assert_eq!(
        xs, expected,
        "all inserted flex_ids must be readable after a byte-size split"
    );
}
