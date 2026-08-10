//! コアロジック（動的シャード分割/統合・被覆・値インデックス・件数）の
//! ランダム差分（モデルベース）検証。
//!
//! 各セルに**一意な値**を割り当てて compaction を抑止し、参照モデル（正解の値）の
//! `HashMap<(x,y), value>` と突き合わせながら挿入/上書き/削除をランダムに繰り返す。
//! 各ラウンド後に次を検証する:
//!   - `data_get`（全域）が正解の値と**完全一致**（データロス・誤値・余剰のいずれも無い）
//!   - `table_count` == 正解の値件数（リーフ件数ヘッダ集計の正しさ）
//!   - `data_filter_eq` が一意値で正しいセルを返す（値インデックスの整合）
//!
//! 挿入過多で `>MAX` 分割を、削除過多で `<閾値` 統合を必ず通過させ、その前後で
//! 被覆（取りこぼし無し）が壊れないことを担保する。

use std::collections::{HashMap, HashSet};

use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::lmdb::initialize_database;
use kasane::repositories::lmdb::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{RangeId, SingleId, SpatialIdSet};

/// 依存を増やさないための決定的 RNG（xorshift64）。
struct XorShift(u64);
impl XorShift {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// `0..n` の一様乱数。
    fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % n as u64) as u32
    }
}

const Z: u8 = 20;
const F: i32 = 0;
/// 66*66 = 4356 セル。`MAX_FLEX_ID_PER_SHARD` を超えるので全セル挿入で必ず分割が起きる。
const W: u32 = 66;
const H: u32 = 66;

fn enc(v: i32) -> Vec<u8> {
    v.to_be_bytes().to_vec()
}
fn dec(b: &[u8]) -> i32 {
    i32::from_be_bytes(b.try_into().unwrap())
}

/// このテーブルのシャードキー数とポインタノードの有無。
fn shard_stats(db: &kasane::repositories::lmdb::AppDb, table_id: TableId) -> (usize, bool) {
    let rtxn = db.env.read_txn().unwrap();
    let mut keys = 0usize;
    let mut has_pointer = false;
    for item in db.tables_data.iter(&rtxn).unwrap() {
        let ((tid, _flex), val) = item.unwrap();
        if tid != table_id {
            continue;
        }
        keys += 1;
        if val.first() == Some(&1) {
            has_pointer = true;
        }
    }
    (keys, has_pointer)
}

/// `data_get`（全域）を `(x,y) -> value` のマップへ復元する。
fn read_all(db: &kasane::repositories::lmdb::AppDb, table_id: TableId) -> HashMap<(u32, u32), i32> {
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), db);
    let mut query = SpatialIdSet::new();
    query.insert(RangeId::new(Z, [F, F], [0, W - 1], [0, H - 1]).unwrap());
    let got = r.data_get_impl(table_id, query, None).unwrap();

    let mut actual = HashMap::new();
    for (value, flex_ids) in got {
        let v = dec(&value);
        for flex_id in flex_ids {
            for sid in flex_id.single_ids() {
                // 一意値なので 1 セルへ展開されるはず。重複は二重被覆のバグを示す。
                let prev = actual.insert((sid.x(), sid.y()), v);
                assert!(
                    prev.is_none(),
                    "cell ({},{}) returned twice (covering overlap?)",
                    sid.x(),
                    sid.y()
                );
            }
        }
    }
    actual
}

/// 正解の値と DB の全状態を突き合わせる。
fn verify(
    db: &kasane::repositories::lmdb::AppDb,
    table_id: TableId,
    dt: TableDataType,
    oracle: &HashMap<(u32, u32), i32>,
    rng: &mut XorShift,
) {
    // 1) data_get 全域が正解の値と完全一致。
    let actual = read_all(db, table_id);
    assert_eq!(
        actual.len(),
        oracle.len(),
        "cell count mismatch: actual={}, oracle={}",
        actual.len(),
        oracle.len()
    );
    assert_eq!(&actual, oracle, "data_get content diverged from the model");

    // 2) table_count（ヘッダ集計）== 正解の値件数。一意値なので compaction されず一致する。
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), db);
    assert_eq!(
        r.table_count_impl(table_id).unwrap(),
        oracle.len() as u64,
        "table_count diverged from the model"
    );

    // 3) 値インデックス: 在のセルを数個サンプルし、その一意値で eq フィルタすると
    //    ちょうどそのセルだけが返る。
    if !oracle.is_empty() {
        let keys: Vec<(u32, u32)> = oracle.keys().copied().collect();
        for _ in 0..8 {
            let &(x, y) = &keys[rng.below(keys.len() as u32) as usize];
            let v = oracle[&(x, y)];
            let hits: HashSet<(u32, u32)> = r
                .data_filter_eq_impl(table_id, dt, &enc(v))
                .unwrap()
                .into_iter()
                .flat_map(|f| f.single_ids().map(|s| (s.x(), s.y())))
                .collect();
            assert_eq!(
                hits,
                HashSet::from([(x, y)]),
                "filter_eq for unique value {v} must return exactly its one cell"
            );
        }
    }
}

#[test]
#[ignore = "heavy: ~thousands of random ops across split/merge"]
fn randomized_model_matches_oracle() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Int;

    let mut rng = XorShift::new(0x1234_5678_9abc_def0);
    let mut oracle: HashMap<(u32, u32), i32> = HashMap::new();
    let mut next_val: i32 = 1;

    // フェーズ A: 全 4356 セルを一意値で確定挿入 → 必ず分割（>MAX）。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        for x in 0..W {
            for y in 0..H {
                let mut set = SpatialIdSet::new();
                set.insert(SingleId::new(Z, F, x, y).unwrap());
                let v = next_val;
                next_val += 1;
                w.data_insert_impl(table_id, dt, set, &enc(v)).unwrap();
                oracle.insert((x, y), v);
            }
        }
        w.commit().unwrap();
    }
    assert!(
        shard_stats(&db, table_id).1,
        "full-grid insert must trigger a split (pointer node)"
    );
    verify(&db, table_id, dt, &oracle, &mut rng);

    // フェーズ B/C: ランダム変異（混在 → 削除過多で統合を誘発）。
    let rounds: &[(u32, u32)] = &[
        (50, 3500), // 混在：分割境界をまたいで挿入/削除
        (10, 6000), // 削除過多：兄弟合算が <閾値 となり統合が走る
    ];
    for &(insert_pct, ops) in rounds {
        {
            let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
            for _ in 0..ops {
                let x = rng.below(W);
                let y = rng.below(H);
                let mut set = SpatialIdSet::new();
                set.insert(SingleId::new(Z, F, x, y).unwrap());

                if rng.below(100) < insert_pct {
                    let v = next_val;
                    next_val += 1;
                    w.data_insert_impl(table_id, dt, set, &enc(v)).unwrap();
                    oracle.insert((x, y), v);
                } else {
                    w.data_remove_impl(table_id, dt, set).unwrap();
                    oracle.remove(&(x, y));
                }
            }
            w.commit().unwrap();
        }
        verify(&db, table_id, dt, &oracle, &mut rng);
    }

    // 残りを全削除 → 完全に空（被覆/統合の終端、値インデックスもクリーン）。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        for &(x, y) in oracle.keys() {
            let mut set = SpatialIdSet::new();
            set.insert(SingleId::new(Z, F, x, y).unwrap());
            w.data_remove_impl(table_id, dt, set).unwrap();
        }
        w.commit().unwrap();
    }
    oracle.clear();

    verify(&db, table_id, dt, &oracle, &mut rng);
    assert_eq!(read_all(&db, table_id).len(), 0, "table must be empty");
    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
    assert_eq!(r.table_count_impl(table_id).unwrap(), 0);

    // 全域のシャードキーが消えている（孤児キーが残っていない）。
    let (keys, _) = shard_stats(&db, table_id);
    assert_eq!(keys, 0, "no orphan shard keys may remain after emptying");
}
