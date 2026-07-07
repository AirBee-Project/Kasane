//! 値インデックス（値フィルタ）のエンドツーエンド検証。
//!
//! 1) 分割（>MAX）を跨いでも等価/範囲フィルタが正しい FlexId を返すこと、
//! 2) 上書き・削除でインデックスが正しく差分維持されること、を確認する。

use std::collections::HashSet;

use kasane::db_init::initialize_database;
use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{IterSingleIds, SingleId, SpatialIdSet};

/// i32 を `interpret_value` と同じ格納形式（ビッグエンディアン）へ。
fn enc(v: i32) -> Vec<u8> {
    v.to_be_bytes().to_vec()
}

/// FlexId 群を、含まれる SingleId の x 集合へ展開する。
fn xs(flex_ids: &[kasane_logic::FlexId]) -> HashSet<u32> {
    flex_ids
        .iter()
        .flat_map(|f| {
            f.clone()
                .iter_single_ids()
                .map(|s| s.x())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
#[ignore]
fn value_filter_eq_and_range_after_split() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Int;

    // 分割閾値(4096)を超える 5000 セルを、各々別の値で挿入する（高カーディナリティ数値）。
    let n: i32 = 5000;
    {
        let wtxn = db.env.write_txn().unwrap();
        let mut w = KasaneDbWrite::new(wtxn, &db);
        for i in 0..n {
            let mut set = SpatialIdSet::new();
            set.insert(SingleId::new(20, 0, (i as u32) * 4, 0).unwrap());
            w.data_insert(table_id, dt, set, &enc(i)).unwrap();
        }
        w.commit().unwrap();
    }

    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);

    let eq = r
        .data_filter_eq(table_id, dt, &enc(1234))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(xs(&eq), HashSet::from([1234u32 * 4]));

    // 範囲: 10 <= value <= 20 → 11 セル。順序保存エンコードが効くことを確認。
    let rng = r
        .data_filter_range(table_id, dt, &enc(10), &enc(20))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected: HashSet<u32> = (10u32..=20).map(|i| i * 4).collect();
    assert_eq!(xs(&rng), expected);
}

#[test]
fn value_filter_reflects_overwrite_and_remove() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Int;
    let id = SingleId::new(20, 0, 100, 0).unwrap();

    let insert = |value: i32| {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut set = SpatialIdSet::new();
        set.insert(id.clone());
        w.data_insert(table_id, dt, set, &enc(value)).unwrap();
        w.commit().unwrap();
    };

    // 値 7 を挿入 → フィルタで引ける。
    insert(7);
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let eq_7 = r
            .data_filter_eq(table_id, dt, &enc(7))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(eq_7.len(), 1);
    }

    // 値 8 で上書き → 旧値 7 は消え、8 が引ける。
    insert(8);
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let eq_7 = r
            .data_filter_eq(table_id, dt, &enc(7))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            eq_7.is_empty(),
            "overwritten value must be gone from the index"
        );
        let eq_8 = r
            .data_filter_eq(table_id, dt, &enc(8))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(eq_8.len(), 1);
    }

    // 削除 → 8 も消える。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut set = SpatialIdSet::new();
        set.insert(id.clone());
        w.data_remove(table_id, dt, set).unwrap();
        w.commit().unwrap();
    }
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let eq_8 = r
            .data_filter_eq(table_id, dt, &enc(8))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(eq_8.is_empty(), "removed value must be gone from the index");
    }
}
