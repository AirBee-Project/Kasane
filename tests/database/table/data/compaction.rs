//! 同値セルの compaction を跨いだ正しさ検証。
//!
//! 同じ値の隣接セルは内部木で粗い FlexId へ畳まれる（compaction）。その状態で
//!   1) 全セルが正しく読み戻せること（被覆・データロス無し）、
//!   2) 値インデックスが compaction 境界を跨いでも整合すること
//!      （上書き・削除で旧値の取りこぼし／残留が起きないこと）。
//!
//! 一意値テスト（model.rs）では通らない compaction 経路を突く。

use std::collections::HashSet;

use kasane::db_init::initialize_database;
use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{RangeId, SpatialIdSet};

const Z: u8 = 20;

fn enc(v: i32) -> Vec<u8> {
    v.to_be_bytes().to_vec()
}

/// `x∈[x0,x1], y∈[y0,y1]`（f=0, z=Z）の矩形を1つの SpatialIdSet にする。
fn rect(x0: u32, x1: u32, y0: u32, y1: u32) -> SpatialIdSet {
    let mut set = SpatialIdSet::new();
    set.insert(RangeId::new(Z, [0, 0], [x0, x1], [y0, y1]).unwrap());
    set
}

fn cells(x0: u32, x1: u32, y0: u32, y1: u32) -> HashSet<(u32, u32)> {
    let mut s = HashSet::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            s.insert((x, y));
        }
    }
    s
}

/// 値 `v` を eq フィルタし、ヒットした全 FlexId を単体セル `(x,y)` 集合へ展開する。
fn filter_cells(db: &kasane::db_init::AppDb, table_id: TableId, v: i32) -> HashSet<(u32, u32)> {
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), db);
    r.data_filter_eq(table_id, TableDataType::Int, &enc(v))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .into_iter()
        .flat_map(|f| f.single_ids().map(|s| (s.x(), s.y())))
        .collect()
}

/// `data_get` で矩形全域を読み、`(x,y) -> value` を復元する。
fn read_rect(
    db: &kasane::db_init::AppDb,
    table_id: TableId,
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
) -> std::collections::HashMap<(u32, u32), i32> {
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), db);
    let got = r.data_get(table_id, rect(x0, x1, y0, y1)).unwrap();
    let mut out = std::collections::HashMap::new();
    for (value, flex_ids) in got {
        let v = i32::from_be_bytes(value.as_slice().try_into().unwrap());
        for f in flex_ids {
            for s in f.single_ids() {
                out.insert((s.x(), s.y()), v);
            }
        }
    }
    out
}

#[test]
fn compaction_roundtrip_and_index_cleanup() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Int;

    // 16x16 = 256 セルを単一値 7 で挿入 → 内部で粗い FlexId へ compaction される。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_insert(table_id, dt, rect(0, 15, 0, 15), &enc(7))
            .unwrap();
        w.commit().unwrap();
    }

    // 実際に compaction が起きている（保持 FlexId 数 < 256）ことを確認。
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let cnt = r.table_count(table_id).unwrap();
        assert!(
            cnt < 256,
            "expected compaction to coarsen below 256 flex ids, got {cnt}"
        );
        assert!(cnt > 0);
    }

    // 1) 全 256 セルが値 7 で読み戻せる（compaction しても取りこぼし無し）。
    let all = read_rect(&db, table_id, 0, 15, 0, 15);
    assert_eq!(all.len(), 256, "all 256 cells must be present");
    assert!(all.values().all(|&v| v == 7));

    // 2) filter_eq(7) は compaction されていてもちょうど 256 セルを被覆する。
    assert_eq!(filter_cells(&db, table_id, 7), cells(0, 15, 0, 15));

    // 左半分(x 0..7)を値 9 で上書き → compaction 境界を跨ぐ差分更新。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_insert(table_id, dt, rect(0, 7, 0, 15), &enc(9))
            .unwrap();
        w.commit().unwrap();
    }

    // 3) 上書き後、値ごとに正しく分かれる（旧値 7 の残留も、新値 9 の欠落も無い）。
    assert_eq!(
        filter_cells(&db, table_id, 9),
        cells(0, 7, 0, 15),
        "overwritten half must filter as value 9"
    );
    assert_eq!(
        filter_cells(&db, table_id, 7),
        cells(8, 15, 0, 15),
        "remaining half must filter as value 7 with no stale entries"
    );
    let mixed = read_rect(&db, table_id, 0, 15, 0, 15);
    assert_eq!(mixed.len(), 256);
    for (&(x, _y), &v) in &mixed {
        assert_eq!(v, if x <= 7 { 9 } else { 7 });
    }

    // 右半分(x 8..15, 値 7)を削除 → 値 7 のインデックスは完全に消える。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_remove(table_id, dt, rect(8, 15, 0, 15)).unwrap();
        w.commit().unwrap();
    }
    assert!(
        filter_cells(&db, table_id, 7).is_empty(),
        "value 7 must be fully gone from the index after removal"
    );
    assert_eq!(
        filter_cells(&db, table_id, 9),
        cells(0, 7, 0, 15),
        "value 9 half must be untouched by the removal"
    );

    // 残りも削除 → 完全に空。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_remove(table_id, dt, rect(0, 7, 0, 15)).unwrap();
        w.commit().unwrap();
    }
    assert!(filter_cells(&db, table_id, 9).is_empty());
    assert_eq!(read_rect(&db, table_id, 0, 15, 0, 15).len(), 0);
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
    assert_eq!(r.table_count(table_id).unwrap(), 0);
}
