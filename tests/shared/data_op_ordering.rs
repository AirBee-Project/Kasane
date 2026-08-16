//! `WriteRepository::data_apply_many` が insert/upsert/remove の混在バッチを、
//! 到着順のまま適用することの検証。
//!
//! `WriteBatch`（[`coalesce`](kasane::services::database::table::data::coalesce)）は
//! 「いつ flush するか」と「到着順の維持」だけを見て、実際の適用はここで検証する
//! `data_apply_many` に任せている。ここでは、1 回の呼び出しで混在バッチを渡した結果が、
//! 同じ順序で 1 件ずつ別トランザクションとして適用した結果と一致することを確かめる。
//! `BatchWrite`（旧: 「最後の insert が勝つ」早見表）のような近道は upsert が絡むと
//! 使えないため、この一致こそが畳み込みの正しさの核心になる。
//!
//! `#[path]` 付きの `mod` で各テストバイナリへ取り込む。名前は `tag` から作る。

#![allow(dead_code)]

use std::collections::HashMap;

use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::{DataOp, ReadRepository, Storage, WriteRepository};
use kasane_logic::{SingleId, SpatialIdSet};

struct Names {
    db: String,
    batched_table: String,
    sequential_table: String,
}

impl Names {
    fn new(tag: &str) -> Self {
        Self {
            db: format!("dataop_{tag}_db"),
            batched_table: format!("dataop_{tag}_batched"),
            sequential_table: format!("dataop_{tag}_sequential"),
        }
    }
}

fn ids_at(x: u32) -> SpatialIdSet {
    let mut ids = SpatialIdSet::new();
    ids.insert(SingleId::new(10, 0, x, 0).unwrap());
    ids
}

/// x=0..4 の 4 つの空間 ID それぞれで、到着順が結果を左右する組み合わせを踏む。
///
/// - x=0: remove → upsert（削除後の upsert は書き込む）
/// - x=1: upsert → remove（先に書いた upsert が消える）
/// - x=2: insert → upsert（upsert は既存値を保つ）
/// - x=3: upsert → insert（insert は既存値を上書きする）
fn scenario() -> Vec<DataOp> {
    vec![
        DataOp::Remove { ids: ids_at(0) },
        DataOp::Upsert {
            ids: ids_at(0),
            value: b"r0-upsert".to_vec(),
        },
        DataOp::Upsert {
            ids: ids_at(1),
            value: b"u1-upsert".to_vec(),
        },
        DataOp::Remove { ids: ids_at(1) },
        DataOp::Insert {
            ids: ids_at(2),
            value: b"i2-insert".to_vec(),
        },
        DataOp::Upsert {
            ids: ids_at(2),
            value: b"i2-upsert-should-not-win".to_vec(),
        },
        DataOp::Upsert {
            ids: ids_at(3),
            value: b"u3-upsert".to_vec(),
        },
        DataOp::Insert {
            ids: ids_at(3),
            value: b"u3-insert-should-win".to_vec(),
        },
    ]
}

async fn create_table<S: Storage>(db: &S, db_name: &str, table_name: &str) -> TableId {
    let create_db_name = db_name.to_string();
    let create_table_name = table_name.to_string();
    db.write(async move |w| {
        // TiKV はクラスタを共有するため、前回の残骸を消してから作る。
        let _ = w.database_remove(&create_db_name).await;
        w.database_create(&create_db_name, None).await?;
        w.table_create(
            &create_db_name,
            &create_table_name,
            TableDataType::Text,
            10,
            None,
            None,
            true,
        )
        .await
    })
    .await
    .unwrap();

    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    db.read(async move |r| r.table_info(&db_name, &table_name).await)
        .await
        .unwrap()
        .unwrap()
        .id
}

/// x=0..4 それぞれの最終値を読み出す。値が無ければキーごと欠落させる。
async fn read_all<S: Storage>(db: &S, table_id: TableId) -> HashMap<u32, Vec<u8>> {
    let mut out = HashMap::new();
    for x in 0..4u32 {
        let groups = db
            .read(async move |r| r.data_get(table_id, ids_at(x), None).await)
            .await
            .unwrap();
        if let Some((value, flex_ids)) = groups.into_iter().next() {
            assert!(!flex_ids.is_empty(), "値はあるのに対象 FlexId が空");
            out.insert(x, value);
        }
    }
    out
}

pub async fn run<S: Storage>(db: &S, tag: &str) {
    let names = Names::new(tag);

    let batched_table = create_table(db, &names.db, &names.batched_table).await;
    let sequential_table = create_table(db, &names.db, &names.sequential_table).await;

    // まとめて 1 回で適用する（WriteBatch が 1 flush でやることと同じ）。
    let ops = scenario();
    db.write(async move |w| {
        w.data_apply_many(batched_table, Some(TableDataType::Text), ops)
            .await
    })
    .await
    .unwrap();

    // 同じ順序で、1 件ずつ別トランザクションとして適用する。
    for op in scenario() {
        db.write(async move |w| {
            w.data_apply_many(sequential_table, Some(TableDataType::Text), vec![op])
                .await
        })
        .await
        .unwrap();
    }

    let batched = read_all(db, batched_table).await;
    let sequential = read_all(db, sequential_table).await;

    assert_eq!(
        batched, sequential,
        "1 回の data_apply_many の結果が、同じ順序で 1 件ずつ適用した結果と食い違っている"
    );

    // 具体的な期待値も確認しておく（読み間違いで両方揃って壊れるのを防ぐ）。
    let mut expected = HashMap::new();
    expected.insert(0, b"r0-upsert".to_vec());
    expected.insert(2, b"i2-insert".to_vec());
    expected.insert(3, b"u3-insert-should-win".to_vec());
    assert_eq!(batched, expected);
}
