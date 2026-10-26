//! 同値の FlexId の compaction を跨いだ正しさ検証。
//!
//! 同じ値の隣接 FlexId は内部木で粗い FlexId へ畳まれる（compaction）。その状態で
//! 全 FlexId が正しく読み戻せること（被覆・データロス無し）を検証する。
//!
//! 一意値テスト（model.rs）では通らない compaction 経路を突く。

use kasane::models::id::TableId;
use kasane::repositories::lmdb::initialize_database;
use kasane::repositories::lmdb::{KasaneDbRead, KasaneDbWrite};
use kasane::repositories::{CatalogRepository, ReadRepository, WriteRepository};
use spatial_id::{RangeId, collection::SpatialIdSet};

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

/// `data_get` で矩形全域を読み、`(x,y) -> value` を復元する。
async fn read_rect(
    db: &kasane::repositories::lmdb::Lmdb,
    table_id: TableId,
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
) -> std::collections::HashMap<(u32, u32), i32> {
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), db);
    let got = r.data_get(table_id, rect(x0, x1, y0, y1)).await.unwrap();
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

#[tokio::test]
async fn compaction_roundtrip() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap()).unwrap();
    let table_id = TableId(uuid::Uuid::now_v7());

    // 16x16 = 256 FlexId を単一値 7 で挿入 → 内部で粗い FlexId へ compaction される。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_insert(table_id, rect(0, 15, 0, 15), &enc(7))
            .await
            .unwrap();
        w.commit().unwrap();
    }

    // 実際に compaction が起きている（保持 FlexId 数 < 256）ことを確認。
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let cnt = r.table_count(table_id).await.unwrap();
        assert!(
            cnt < 256,
            "expected compaction to coarsen below 256 flex ids, got {cnt}"
        );
        assert!(cnt > 0);
    }

    // 1) 全 256 FlexId が値 7 で読み戻せる（compaction しても取りこぼし無し）。
    let all = read_rect(&db, table_id, 0, 15, 0, 15).await;
    assert_eq!(all.len(), 256, "all 256 flex_ids must be present");
    assert!(all.values().all(|&v| v == 7));

    // 左半分(x 0..7)を値 9 で上書き → compaction 境界を跨ぐ差分更新。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_insert(table_id, rect(0, 7, 0, 15), &enc(9))
            .await
            .unwrap();
        w.commit().unwrap();
    }

    // 2) 上書き後、値ごとに正しく分かれる（旧値 7 の残留も、新値 9 の欠落も無い）。
    let mixed = read_rect(&db, table_id, 0, 15, 0, 15).await;
    assert_eq!(mixed.len(), 256);
    for (&(x, _y), &v) in &mixed {
        assert_eq!(v, if x <= 7 { 9 } else { 7 });
    }

    // 右半分(x 8..15, 値 7)を削除。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_remove(table_id, rect(8, 15, 0, 15)).await.unwrap();
        w.commit().unwrap();
    }
    let left = read_rect(&db, table_id, 0, 15, 0, 15).await;
    assert_eq!(left.len(), 128);
    for (&(x, _y), &v) in &left {
        assert_eq!(v, 9);
        assert!(x <= 7);
    }

    // 残りも削除 → 完全に空。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        w.data_remove(table_id, rect(0, 7, 0, 15)).await.unwrap();
        w.commit().unwrap();
    }
    assert_eq!(read_rect(&db, table_id, 0, 15, 0, 15).await.len(), 0);
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
    assert_eq!(r.table_count(table_id).await.unwrap(), 0);
}
