//! TiKV バックエンドの結合テスト。
//!
//! 実際の TiKV クラスタが必要なので、`backend-tikv` を有効にしたときだけコンパイルされる。
//!
//! ```bash
//! docker compose -f deployment/tikv/docker-compose.yml up -d
//! cargo test --no-default-features --features backend-tikv --test tikv_backend
//! ```
//!
//! 各テストは自分専用の名前空間（ユニークなデータベース名）を使うので、
//! 同じクラスタに対して並行して実行してもよい。

#![cfg(feature = "backend-tikv")]

use kasane::error::AppError;
use kasane::models::database::table::{Table, TableDataType};
use kasane::models::id::TableId;
use kasane::repositories::tikv::{GcConfig, TikvConfig, TikvDb};
use kasane::repositories::{ReadRepository, Storage, WriteRepository};
use kasane_logic::{SingleId, SpatialIdSet};

async fn connect() -> TikvDb {
    TikvDb::connect(TikvConfig::from_env())
        .await
        .expect("cannot reach TiKV; start the cluster with ")
}

/// テストごとに衝突しないデータベース名。
fn unique_db(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::now_v7().simple())
}

/// 後始末。失敗しても無視する（テストの主目的ではないため）。
async fn drop_db(db: &TikvDb, name: &str) {
    let name = name.to_string();
    let _ = db
        .write(async move |w| w.database_remove(&name).await)
        .await;
}

/// データベースと、その中の `Int` テーブル 1 つを作る。
async fn make_table(db: &TikvDb, db_name: &str) -> Table {
    let db_name = db_name.to_string();
    db.write(async move |w| {
        w.database_create(&db_name, None).await?;
        w.table_create(
            &db_name,
            "t",
            TableDataType::Int,
            25,
            None,
            None,
            true,
            false,
        )
        .await
    })
    .await
    .unwrap()
}

/// `xs` の各値を `(20, f, x, y)` の FlexId として書き込む。
///
/// 1 トランザクションが大きくなりすぎないようチャンクに分ける。値は `x` そのもの。
async fn insert_flex_ids(
    db: &TikvDb,
    table_id: TableId,
    xs: impl IntoIterator<Item = u32>,
    f: i32,
    y: u32,
) {
    const CHUNK: usize = 500;
    let xs: Vec<u32> = xs.into_iter().collect();
    for chunk in xs.chunks(CHUNK) {
        let chunk = chunk.to_vec();
        db.write(async move |w| {
            for &x in &chunk {
                let mut ids = SpatialIdSet::new();
                ids.insert(SingleId::new(20, f, x, y).unwrap());
                w.data_insert(
                    table_id,
                    Some(TableDataType::Int),
                    ids,
                    &(x as i64).to_be_bytes(),
                )
                .await?;
            }
            Ok(())
        })
        .await
        .unwrap();
    }
}

/// 2 つのインスタンスから並行に 1 FlexId ずつ書き込み、両方の完了を待つ。
///
/// `x` は `(writer 番号, i)` から決める。同じリーフを狙うか離すかを呼び出し側が選べる。
async fn insert_from_both(
    writers: [&TikvDb; 2],
    table_id: TableId,
    per_writer: u32,
    y: u32,
    x_of: fn(usize, u32) -> u32,
) {
    let mut handles = Vec::new();
    for (which, db) in writers.into_iter().enumerate() {
        let db = db.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..per_writer {
                let x = x_of(which, i);
                let db = db.clone();
                let mut ids = SpatialIdSet::new();
                ids.insert(SingleId::new(20, 0, x, y).unwrap());
                db.write(async move |w| {
                    w.data_insert(
                        table_id,
                        Some(TableDataType::Int),
                        ids.clone(),
                        &(x as i64).to_be_bytes(),
                    )
                    .await
                })
                .await
                .unwrap();
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
}

/// 指定した FlexId 群のうち、実際に読み戻せた件数。
async fn count_readable(db: &TikvDb, table_id: TableId, ids: SpatialIdSet) -> usize {
    let groups = db
        .read(async move |r| r.data_get(table_id, ids.clone()).await)
        .await
        .unwrap();
    groups.iter().map(|(_, ids)| ids.len()).sum()
}

/// `(20, f, x, y)` の FlexId 集合。
fn flex_ids(xs: impl IntoIterator<Item = u32>, f: i32, y: u32) -> SpatialIdSet {
    let mut set = SpatialIdSet::new();
    for x in xs {
        set.insert(SingleId::new(20, f, x, y).unwrap());
    }
    set
}

#[tokio::test]
async fn database_lifecycle() {
    let db = connect().await;
    let name = unique_db("lifecycle");

    let created = {
        let name = name.clone();
        db.write(async move |w| w.database_create(&name, Some("hello".into())).await)
            .await
            .unwrap()
    };
    assert_eq!(created.name, name);

    // 別トランザクションから見えること（＝コミットされていること）。
    let info = {
        let name = name.clone();
        db.read(async move |r| Ok(r.database_info(&name).await?.map(|(_, info)| info)))
            .await
            .unwrap()
    };
    assert_eq!(
        info.expect("created database").description.as_deref(),
        Some("hello")
    );

    // 同名の再作成は弾かれる。
    let dup = {
        let name = name.clone();
        db.write(async move |w| w.database_create(&name, None).await)
            .await
    };
    assert!(matches!(
        dup,
        Err(AppError::AlreadyExists {
            resource: kasane::error::Resource::Database,
            ..
        })
    ));

    drop_db(&db, &name).await;

    let gone = {
        let name = name.clone();
        db.read(async move |r| Ok(r.database_info(&name).await?.map(|(_, info)| info)))
            .await
            .unwrap()
    };
    assert!(gone.is_none(), "削除後も残っている");
}

#[tokio::test]
async fn closure_error_discards_the_transaction() {
    let db = connect().await;
    let name = unique_db("rollback");

    let result: Result<(), AppError> = {
        let name = name.clone();
        db.write(async move |w| {
            w.database_create(&name, None).await?;
            Err(AppError::InternalError("boom".into()))
        })
        .await
    };
    assert!(result.is_err());

    let info = {
        let name = name.clone();
        db.read(async move |r| Ok(r.database_info(&name).await?.map(|(_, info)| info)))
            .await
            .unwrap()
    };
    assert!(
        info.is_none(),
        "失敗したトランザクションがコミットされている"
    );
}

#[tokio::test]
async fn table_and_data_roundtrip() {
    let db = connect().await;
    let name = unique_db("data");

    let table = make_table(&db, &name).await;

    // 値を書いて、読み戻せること。
    let mut ids = SpatialIdSet::new();
    ids.insert(SingleId::new(20, 0, 100, 200).unwrap());
    let value = 1234i64.to_be_bytes().to_vec();

    {
        let ids = ids.clone();
        let value = value.clone();
        db.write(async move |w| {
            w.data_insert(table.id, Some(TableDataType::Int), ids.clone(), &value)
                .await
        })
        .await
        .unwrap();
    }

    let groups = {
        let ids = ids.clone();
        db.read(async move |r| r.data_get(table.id, ids.clone()).await)
            .await
            .unwrap()
    };
    assert_eq!(groups.len(), 1, "1 種類の値が返るはず");
    assert_eq!(groups[0].0, value);
    assert_eq!(groups[0].1.len(), 1);

    // 件数と値インデックスも一致すること。
    let count = db
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let hits = {
        let value = value.clone();
        db.read(async move |r| r.data_filter_eq(table.id, TableDataType::Int, &value).await)
            .await
            .unwrap()
    };
    assert_eq!(hits.len(), 1, "値インデックスから引けない");

    // 削除すると、データも値インデックスも消えること。
    {
        let ids = ids.clone();
        db.write(async move |w| {
            w.data_remove(table.id, Some(TableDataType::Int), ids.clone())
                .await
        })
        .await
        .unwrap();
    }

    let count = db
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(count, 0);

    let hits = db
        .read(async move |r| r.data_filter_eq(table.id, TableDataType::Int, &value).await)
        .await
        .unwrap();
    assert!(hits.is_empty(), "削除後も値インデックスに残っている");

    drop_db(&db, &name).await;
}

/// シャードの分割が起きる規模を書き込んでも、全 FlexId が読み戻せること。
#[tokio::test]
async fn shard_split_preserves_all_flex_ids() {
    let db = connect().await;
    let name = unique_db("split");

    let table = make_table(&db, &name).await;

    // MAX_FLEX_ID_PER_SHARD を超える数の FlexId を、それぞれ別の値で入れる。
    const N: u32 = 1500;
    insert_flex_ids(&db, table.id, 0..N, 0, 0).await;

    let count = db
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(count, N as u64, "分割後に FlexId が失われている");

    // 個別の値がインデックスから正しく引けること（分割境界を跨いでも壊れない）。
    for probe in [0u32, 777, N - 1] {
        let value = (probe as i64).to_be_bytes().to_vec();
        let hits = db
            .read(async move |r| r.data_filter_eq(table.id, TableDataType::Int, &value).await)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "値 {probe} が値インデックスから引けない");
    }

    drop_db(&db, &name).await;
}

/// **可変長（Text）の範囲検索が、境界を絞っても取りこぼさないこと。**
///
/// キーは `0x07 ‖ table_id ‖ 値 ‖ flexid` と値を可変長のまま連結するので、該当キーは
/// バイト順で連続しない。`vkey` が上限の真の接頭辞になっている行は、続く flexid の
/// バイト次第で `hi ‖ 0xFF…` を超えた位置へ飛ぶ。走査範囲を詰めすぎると、
/// そういう行だけが黙って消える。
///
/// LMDB 側にも同等のテストがあるが、こちらは終端付き範囲スキャンという
/// 別の下位機構を通るので、別に確かめる。
#[tokio::test]
async fn text_range_filter_keeps_values_that_prefix_the_upper_bound() {
    let db = connect().await;
    let name = unique_db("text_range");

    let table = {
        let name = name.clone();
        db.write(async move |w| {
            w.database_create(&name, None).await?;
            w.table_create(&name, "t", TableDataType::Text, 25, None, None, true, false)
                .await
        })
        .await
        .unwrap()
    };

    // 値と、それを置く FlexId の x 座標。
    let rows: [(&str, u32); 6] = [
        ("a", 1),   // 範囲外（下限未満）
        ("b", 2),   // 該当。"bz" の真の接頭辞 ＝ 取りこぼしやすい行
        ("bm", 3),  // 該当
        ("bz", 4),  // 該当（上限そのもの）
        ("bza", 5), // 範囲外（上限を超える）
        ("c", 6),   // 範囲外
    ];
    for (value, x) in rows {
        let value = value.as_bytes().to_vec();
        let mut ids = SpatialIdSet::new();
        ids.insert(SingleId::new(20, 0, x, 0).unwrap());
        db.write(async move |w| {
            w.data_insert(table.id, Some(TableDataType::Text), ids.clone(), &value)
                .await
        })
        .await
        .unwrap();
    }

    /// 引けた FlexId 群を x 座標の集合へ。
    async fn xs_of(db: &TikvDb, table_id: TableId, lo: &str, hi: &str) -> Vec<u32> {
        let (lo, hi) = (lo.as_bytes().to_vec(), hi.as_bytes().to_vec());
        let hits = db
            .read(async move |r| {
                r.data_filter_range(table_id, TableDataType::Text, &lo, &hi)
                    .await
            })
            .await
            .unwrap();
        let mut xs: Vec<u32> = hits
            .iter()
            .flat_map(|f| (*f).single_ids().map(|s| s.x()))
            .collect();
        xs.sort_unstable();
        xs
    }

    assert_eq!(
        xs_of(&db, table.id, "b", "bz").await,
        vec![2, 3, 4],
        "上限の接頭辞になる値を取りこぼしたか、範囲外を拾っている"
    );
    assert_eq!(xs_of(&db, table.id, "a", "c").await, vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(xs_of(&db, table.id, "bm", "bm").await, vec![3]);
    assert!(
        xs_of(&db, table.id, "z", "a").await.is_empty(),
        "空範囲が何かを返している"
    );

    drop_db(&db, &name).await;
}

/// **同一リーフを複数インスタンスから同時に書いても、更新が失われないこと。**
///
/// 近接した空間 ID は同じシャード（1 キー）に入るので、これは「同じキーへの
/// read-modify-write を並行で走らせる」テストになる。リーフ単位ロックが効いていなければ、
/// 後勝ちで互いの挿入を上書きし合って件数が合わなくなる。
///
/// `separate_instances_stay_consistent` は FlexId が散らばるのでリーフが分かれうるが、
/// こちらは意図的に 1 リーフへ集中させている。
#[tokio::test]
async fn concurrent_writers_on_the_same_leaf_do_not_lose_updates() {
    let a = connect().await;
    let b = connect().await;
    let name = unique_db("same_leaf");

    let table = make_table(&a, &name).await;

    // 隣接する 1 本の線上に並べる。分割閾値（1024）より十分小さいので 1 リーフに収まる。
    const PER_WRITER: u32 = 40;
    insert_from_both([&a, &b], table.id, PER_WRITER, 0, |which, i| {
        i * 2 + which as u32
    })
    .await;

    let count = b
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(
        count,
        (PER_WRITER * 2) as u64,
        "同一リーフへの並行書き込みで更新が失われた"
    );

    // 実際に全 FlexId が引けること（件数だけ合っていても意味がない）。
    let mut all = SpatialIdSet::new();
    for x in 0..PER_WRITER * 2 {
        all.insert(SingleId::new(20, 0, x, 0).unwrap());
    }
    let groups = a
        .read(async move |r| r.data_get(table.id, all.clone()).await)
        .await
        .unwrap();
    let actual: usize = groups.iter().map(|(_, ids)| ids.len()).sum();
    assert_eq!(
        actual,
        (PER_WRITER * 2) as usize,
        "読み戻せない FlexId がある"
    );

    drop_db(&a, &name).await;
}

/// **別のリーフへの書き込みが互いをブロックしないこと。**
///
/// テーブル全体ロックが残っていると、離れた領域への書き込みでも直列化される。
/// 空間的に大きく離した 2 つの書き込み列を同時に流し、両方が完了することを見る。
/// （時間で判定すると環境差で不安定になるので、ここでは「デッドロックや
/// リトライ枯渇を起こさずに完了すること」を確かめる。）
#[tokio::test]
async fn writes_to_distant_regions_make_progress_together() {
    let a = connect().await;
    let b = connect().await;
    let name = unique_db("distant");

    let table = make_table(&a, &name).await;

    // 先に木を育てて、離れた領域が別リーフになるようにする。
    insert_flex_ids(&a, table.id, (0..2000u32).map(|i| i * 400), 0, 0).await;

    const PER_WRITER: u32 = 30;
    // 2 つの書き込み列を大きく離す。
    const BASES: [u32; 2] = [1_000_000, 500];
    insert_from_both([&a, &b], table.id, PER_WRITER, 1, |which, i| {
        BASES[which] + i
    })
    .await;

    for base in BASES {
        let got = count_readable(&a, table.id, flex_ids(base..base + PER_WRITER, 0, 1)).await;
        assert_eq!(
            got, PER_WRITER as usize,
            "base={base} の書き込みが欠けている"
        );
    }

    drop_db(&a, &name).await;
}

/// **テーブル削除は論理削除で、実体は GC が回収すること。**
#[tokio::test]
async fn removing_a_table_retires_it_and_gc_reclaims_the_data() {
    let db = connect().await;
    let name = unique_db("retire");

    let table = make_table(&db, &name).await;

    insert_flex_ids(&db, table.id, 0..1500u32, 0, 3).await;
    assert_eq!(
        db.read(async move |r| r.table_count(table.id).await)
            .await
            .unwrap(),
        1500
    );

    // 削除はカタログからの除去だけで完了する。
    {
        let name = name.clone();
        db.write(async move |w| w.table_remove(&name, "t").await)
            .await
            .unwrap();
    }
    let listed = {
        let name = name.clone();
        db.read(async move |r| r.table_list(&name).await)
            .await
            .unwrap()
    };
    assert!(listed.is_empty(), "削除したテーブルが一覧に残っている");

    // 回収を回すと実体が消える。何度回しても壊れない（冪等）。
    db.sweep_retired_tables(&GcConfig::eager()).await.unwrap();
    db.sweep_retired_tables(&GcConfig::eager()).await.unwrap();

    assert_eq!(
        db.read(async move |r| r.table_count(table.id).await)
            .await
            .unwrap(),
        0,
        "回収後もシャードが残っている"
    );

    drop_db(&db, &name).await;
}

/// **件数索引が、シャード本体から数えた実数と一致し続けること。**
///
/// `table_count` は件数だけを切り出した索引（`0x08`）を読む。本体から数えないので速い
/// 代わりに、シャードを書き換える経路のどれかが索引の更新を落とすと黙ってずれる。
/// 分割・統合・削除・複製をひととおり通したうえで、実数と突き合わせる。
#[tokio::test]
async fn the_count_index_tracks_the_shard_entries() {
    let db = connect().await;
    let name = unique_db("count_index");

    let table = make_table(&db, &name).await;

    // 分割が起きる規模まで入れる。
    const N: u32 = 1500;
    insert_flex_ids(&db, table.id, 0..N, 0, 0).await;
    assert_eq!(
        db.read(async move |r| r.table_count(table.id).await)
            .await
            .unwrap(),
        N as u64,
        "挿入後の件数がずれている"
    );

    // 統合が起きる規模まで消す。
    const REMOVED: u32 = 1200;
    for chunk_start in (0..REMOVED).step_by(400) {
        let chunk: Vec<u32> = (chunk_start..(chunk_start + 400).min(REMOVED)).collect();
        db.write(async move |w| {
            for &i in &chunk {
                let mut ids = SpatialIdSet::new();
                ids.insert(SingleId::new(20, 0, i, 0).unwrap());
                w.data_remove(table.id, Some(TableDataType::Int), ids)
                    .await?;
            }
            Ok(())
        })
        .await
        .unwrap();
    }
    let after_removal = db
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(
        after_removal,
        (N - REMOVED) as u64,
        "削除・統合後の件数がずれている"
    );

    // 実際に読める FlexId 数とも一致すること（索引だけが正しくても意味がない）。
    let actual = count_readable(&db, table.id, flex_ids(0..N, 0, 0)).await;
    assert_eq!(
        actual as u64, after_removal,
        "件数索引と実際に読める FlexId 数が食い違っている"
    );

    // 複製先にも件数が引き継がれること。
    let copy = {
        let name = name.clone();
        db.write(async move |w| w.table_copy(&name, "t", &name, "t_copy").await)
            .await
            .unwrap()
    };
    assert_eq!(
        db.read(async move |r| r.table_count(copy.id).await)
            .await
            .unwrap(),
        after_removal,
        "複製したテーブルの件数が引き継がれていない"
    );

    drop_db(&db, &name).await;
}

/// 並行書き込みが直列化され、更新が失われないこと。
#[tokio::test]
async fn concurrent_writes_do_not_lose_updates() {
    let db = connect().await;
    let name = unique_db("concurrent");

    let table = make_table(&db, &name).await;

    // 同一テーブルへ別の FlexId を並行に書き込む。テーブルスコープのロックを奪い合う。
    const WRITERS: u32 = 8;
    let mut handles = Vec::new();
    for i in 0..WRITERS {
        let db = db.clone();
        handles.push(tokio::spawn(async move {
            let mut ids = SpatialIdSet::new();
            ids.insert(SingleId::new(20, 0, i, 0).unwrap());
            db.write(async move |w| {
                w.data_insert(
                    table.id,
                    Some(TableDataType::Int),
                    ids.clone(),
                    &(i as i64).to_be_bytes(),
                )
                .await
            })
            .await
            .unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let count = db
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(
        count, WRITERS as u64,
        "並行書き込みで更新が失われた（直列化できていない）"
    );

    drop_db(&db, &name).await;
}

/// **複数の Kasane インスタンスが同じクラスタへ同時に書いても整合性が保たれること。**
///
/// 排他はすべて TiKV 側の悲観ロックで行っており、プロセスローカルなロックは持たない。
/// それを実証するため、独立した [`TikvDb`]（＝別々の接続）を 2 つ用意し、
/// 同一テーブルへ同時に書き込む。クライアント側で直列化しているなら、
/// 別インスタンス同士では効かず件数が合わなくなる。
#[tokio::test]
async fn separate_instances_stay_consistent() {
    // 2 つの Kasane プロセスに相当する、独立した接続。
    let a = connect().await;
    let b = connect().await;
    let name = unique_db("multi_instance");

    let table = make_table(&a, &name).await;

    // 同じテーブルへ、両インスタンスから交互の FlexId を同時に書く。
    const PER_INSTANCE: u32 = 12;
    let mut handles = Vec::new();
    for (offset, db) in [(0u32, a.clone()), (1u32, b.clone())] {
        handles.push(tokio::spawn(async move {
            for i in 0..PER_INSTANCE {
                let x = i * 2 + offset;
                let db = db.clone();
                let mut ids = SpatialIdSet::new();
                ids.insert(SingleId::new(20, 0, x, 0).unwrap());
                db.write(async move |w| {
                    w.data_insert(
                        table.id,
                        Some(TableDataType::Int),
                        ids.clone(),
                        &(x as i64).to_be_bytes(),
                    )
                    .await
                })
                .await
                .unwrap();
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    // 片方のインスタンスから読んで、両者の書き込みが漏れなく見えること。
    let count = b
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(
        count,
        (PER_INSTANCE * 2) as u64,
        "インスタンスを跨いだ書き込みで更新が失われた（排他がプロセス内に閉じている）"
    );

    // 値インデックスも両者ぶん揃っていること（シャード更新の差分が競合していない）。
    for probe in [0u32, 1, PER_INSTANCE * 2 - 1] {
        let value = (probe as i64).to_be_bytes().to_vec();
        let hits = a
            .read(async move |r| r.data_filter_eq(table.id, TableDataType::Int, &value).await)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "値 {probe} が値インデックスから引けない");
    }

    drop_db(&a, &name).await;
}

/// 既定ユーザーの投入まわりの契約。
///
/// クラスタ全体で 1 つしかない `root` を触るので、他のテストと競合しないよう
/// 1 つのテストにまとめてある（分けると並列実行で互いの前提を壊す）。
///
/// 1. 複数インスタンスが同時起動しても二重に作られない
/// 2. **削除した管理者が、再起動で既定パスワードのまま復活しない**
///
/// 2 が要点。投入を「`root` が居なければ作る」で判定すると、意図して消した管理者が
/// 次の起動で `ROOT_PASSWORD`（未設定なら既定値）のまま戻ってくる。それは消せない
/// 管理者アカウント＝常設のバックドアなので、初期化済みマーカーで判定して
/// 「投入は文字どおり 1 度きり」にしてある。
#[tokio::test]
async fn the_default_admin_is_seeded_exactly_once_and_never_resurrected() {
    use kasane::models::users::{PrivilegeRule, UserRole};

    // 同時接続。どちらも初期化済みマーカーを見て投入しようとする。
    let (a, b) = tokio::join!(connect(), connect());

    let users = a
        .read(async |r| r.list_users(None, 1000).await)
        .await
        .unwrap();
    let roots = users.iter().filter(|(name, _)| name == "root").count();
    assert_eq!(roots, 1, "root ユーザーが重複して作られている");

    // どちらのインスタンスからも同じ root が見えること。
    let from_b = b.read(async |r| r.get_user("root").await).await.unwrap();
    assert!(from_b.is_some(), "別インスタンスから root が見えない");

    // --- 削除しても復活しないこと ---

    a.write(async |w| w.delete_user("root").await)
        .await
        .unwrap();

    // 「再起動」に相当する新しい接続。
    let restarted = connect().await;
    let after = restarted
        .read(async |r| r.get_user("root").await)
        .await
        .unwrap();
    assert!(
        after.is_none(),
        "削除した root が再起動で復活した（初期化判定が root の有無に依存している）"
    );

    // 後片付け: 次回の実行のために管理者を戻す。
    restarted
        .write(async |w| {
            w.create_user(
                "root",
                kasane::models::id::PrincipalId(uuid::Uuid::now_v7()),
                kasane::services::auth::hash_password("password").unwrap(),
                &[PrivilegeRule::Global {
                    role: UserRole::Admin,
                }],
            )
            .await
        })
        .await
        .unwrap();
}

/// **書き込みクロージャが複数回呼ばれること、そして捨てられた試行が commit されないこと。**
///
/// `Storage::write` はロックが足りない試行を巻き戻してやり直す。この「やり直し」は
/// TiKV でしか起きないので、LMDB のテストを全部通っても再実行前提が守られているとは
/// 限らない。契約そのものをここで固定する。
///
/// `table_remove` は 2 段階でロックが判明する（DB スコープを取って初めて `TableId` が
/// 判る）ので、必ず 2 回以上クロージャが走る。
#[tokio::test]
async fn the_write_closure_is_re_run_until_its_locks_are_held() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let db = connect().await;
    let name = unique_db("retry_contract");

    {
        let name = name.clone();
        db.write(async move |w| {
            w.database_create(&name, None).await?;
            w.table_create(&name, "t", TableDataType::Int, 25, None, None, true, false)
                .await
        })
        .await
        .unwrap();
    }

    let attempts = Arc::new(AtomicUsize::new(0));

    {
        let name = name.clone();
        let attempts = attempts.clone();
        db.write(async move |w| {
            attempts.fetch_add(1, Ordering::SeqCst);
            w.table_remove(&name, "t").await
        })
        .await
        .unwrap();
    }

    assert!(
        attempts.load(Ordering::SeqCst) >= 2,
        "クロージャが 1 回しか呼ばれていない。ロックの二段階宣言が働いていないか、\
         やり直しの契約が変わっている（呼ばれた回数: {})",
        attempts.load(Ordering::SeqCst)
    );

    // 捨てられた試行があっても、結果は 1 回ぶんだけ適用されていること。
    let remaining = {
        let name = name.clone();
        db.read(async move |r| r.table_list(&name).await)
            .await
            .unwrap()
    };
    assert!(remaining.is_empty(), "テーブルが削除されていない");

    drop_db(&db, &name).await;
}

/// 捨てられた試行の書き込みがコミットされないこと。
///
/// ロック不足でやり直した 1 周目にも `data_insert` などは走りうる（宣言より前に
/// 部分的に処理が進む操作があれば）。その周回が commit されないことを、
/// 「途中でエラーを返した書き込みは何も残さない」形で確かめる。
#[tokio::test]
async fn a_failed_closure_leaves_nothing_behind() {
    let db = connect().await;
    let name = unique_db("discarded");

    let table = make_table(&db, &name).await;

    let mut ids = SpatialIdSet::new();
    ids.insert(SingleId::new(20, 0, 7, 7).unwrap());

    let failed = db
        .write(async move |w| {
            w.data_insert(
                table.id,
                Some(TableDataType::Int),
                ids.clone(),
                &1i64.to_be_bytes(),
            )
            .await?;
            // 書いたあとで失敗する。ここまでの変更は捨てられなければならない。
            Err::<(), _>(AppError::InternalError("deliberate failure".to_string()))
        })
        .await;
    assert!(failed.is_err(), "意図的な失敗が伝わっていない");

    let count = db
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(count, 0, "失敗した書き込みの FlexId が残っている");

    drop_db(&db, &name).await;
}

#[tokio::test]
async fn user_privileges_roundtrip() {
    use kasane::models::users::{DataRole, PrivilegeRule};
    use kasane::repositories::CatalogRepository;

    let db = connect().await;
    let name = unique_db("perm");
    let username = format!("u_{}", uuid::Uuid::now_v7().simple());

    {
        let name = name.clone();
        db.write(async move |w| w.database_create(&name, None).await)
            .await
            .unwrap();
    }

    {
        let username = username.clone();
        let name = name.clone();
        db.write(async move |w| {
            w.create_user(
                &username,
                kasane::models::id::PrincipalId(uuid::Uuid::now_v7()),
                "hash".to_string(),
                &[PrivilegeRule::Database {
                    db_name: name.clone(),
                    role: DataRole::Read,
                }],
            )
            .await
        })
        .await
        .unwrap();
    }

    let rendered = {
        let username = username.clone();
        db.read(async move |r| {
            let record = r.require_user_record(&username).await?;
            let entries = r.acl_entries(record.id).await?;
            r.render_privileges(record.global_role, &entries).await
        })
        .await
        .unwrap()
    };
    assert_eq!(
        rendered,
        vec![PrivilegeRule::Database {
            db_name: name.clone(),
            role: DataRole::Read,
        }]
    );

    {
        let username = username.clone();
        db.write(async move |w| w.delete_user(&username).await)
            .await
            .unwrap();
    }

    drop_db(&db, &name).await;
}

#[path = "shared/acl_suite.rs"]
mod acl_suite;

/// ACL 行の保存が仕様どおりか（LMDB 側は `storage.rs` が同じスイートを回す）。
///
/// 名前はタグから作るので、同じクラスタに対して他のテストと並行に走ってよい。
#[tokio::test]
async fn acl_rows_behave_as_specified() {
    let db = connect().await;
    acl_suite::run(&db, &uuid::Uuid::now_v7().simple().to_string()).await;
}

#[tokio::test]
async fn acl_driven_listing_behaves_as_specified() {
    let db = connect().await;
    acl_suite::run_listing(&db, &uuid::Uuid::now_v7().simple().to_string()).await;
}

#[path = "shared/db_compat.rs"]
mod db_compat;

/// **現行スキーマ版で書かれた生バイト列が、今のコードでも記録どおりに読めること。**
///
/// LMDB 側の `db_compat_lmdb` と違い、ここでは「版が違うフィクスチャを注入して
/// `SchemaVersionMismatch` を確かめる」検証はしない。TiKV の版マーカー
/// （`cluster_initialized` / `schema_version`）はクラスタ全体で 1 つしかなく、この
/// バイナリの全テストが 1 クラスタを共有しているため、それを書き換えると他の並行
/// テストの前提まで壊れてしまう。版チェックの分岐そのもの（形は LMDB 側と同じ）は
/// `db_compat_lmdb` が確認しているので、ここでは現行版のデータ往復だけを見る。
#[tokio::test]
async fn current_schema_fixture_stays_readable() {
    let fixtures = db_compat::load_all_fixtures("tikv");
    let Some(fixture) = fixtures
        .into_iter()
        .find(|f| f.schema_version == kasane::repositories::SCHEMA_VERSION)
    else {
        eprintln!(
            "tests/fixtures/db_compat/tikv/v{}/fixture.json が無いのでスキップする。\
             gen_db_compat_fixture で生成してコミットすること",
            kasane::repositories::SCHEMA_VERSION
        );
        return;
    };

    let config = kasane::repositories::tikv::TikvConfig::from_env();
    let raw = tikv_client::TransactionClient::new(config.pd_endpoints.clone())
        .await
        .expect("cannot open a raw client to TiKV");
    db_compat::load_tikv(&raw, &fixture.entries).await;

    let db = connect().await;
    let actual = db_compat::read_actual(&db).await;
    assert_eq!(
        actual, fixture.expected,
        "v{} のフィクスチャを読んだ結果が記録と食い違っている。意図した変更なら \
         SCHEMA_VERSION を上げること",
        fixture.schema_version
    );
}
