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
use kasane::models::database::table::TableDataType;
use kasane::repositories::tikv::{TikvConfig, TikvDb};
use kasane::repositories::{ReadRepository, Storage, WriteRepository};
use kasane_logic::{SingleId, SpatialIdSet};

async fn connect() -> TikvDb {
    TikvDb::connect(TikvConfig::from_env())
        .await
        .expect("TiKV に接続できない。deployment/tikv/docker-compose.yml を起動しているか確認")
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
        db.read(async move |r| r.database_info(&name).await)
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
    assert!(matches!(dup, Err(AppError::DatabaseAlreadyExists { .. })));

    drop_db(&db, &name).await;

    let gone = {
        let name = name.clone();
        db.read(async move |r| r.database_info(&name).await)
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
        db.read(async move |r| r.database_info(&name).await)
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

    let table = {
        let name = name.clone();
        db.write(async move |w| {
            w.database_create(&name, None).await?;
            w.table_create(&name, "t", TableDataType::Int, 25, None, None)
                .await
        })
        .await
        .unwrap()
    };

    // 値を書いて、読み戻せること。
    let mut ids = SpatialIdSet::new();
    ids.insert(SingleId::new(20, 0, 100, 200).unwrap());
    let value = 1234i64.to_be_bytes().to_vec();

    {
        let ids = ids.clone();
        let value = value.clone();
        db.write(async move |w| {
            w.data_insert(table.id, TableDataType::Int, ids.clone(), &value)
                .await
        })
        .await
        .unwrap();
    }

    let groups = {
        let ids = ids.clone();
        db.read(async move |r| r.data_get(table.id, ids.clone(), None).await)
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
            w.data_remove(table.id, TableDataType::Int, ids.clone())
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

/// シャードの分割が起きる規模を書き込んでも、全セルが読み戻せること。
#[tokio::test]
async fn shard_split_preserves_all_cells() {
    let db = connect().await;
    let name = unique_db("split");

    let table = {
        let name = name.clone();
        db.write(async move |w| {
            w.database_create(&name, None).await?;
            w.table_create(&name, "t", TableDataType::Int, 25, None, None)
                .await
        })
        .await
        .unwrap()
    };

    // MAX_FLEX_ID_PER_SHARD (1024) を超える数のセルを、それぞれ別の値で入れる。
    const N: u32 = 1500;
    for chunk_start in (0..N).step_by(500) {
        let chunk: Vec<u32> = (chunk_start..(chunk_start + 500).min(N)).collect();
        db.write(async move |w| {
            for &i in &chunk {
                let mut ids = SpatialIdSet::new();
                ids.insert(SingleId::new(20, 0, i, 0).unwrap());
                w.data_insert(table.id, TableDataType::Int, ids, &(i as i64).to_be_bytes())
                    .await?;
            }
            Ok(())
        })
        .await
        .unwrap();
    }

    let count = db
        .read(async move |r| r.table_count(table.id).await)
        .await
        .unwrap();
    assert_eq!(count, N as u64, "分割後にセルが失われている");

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

/// 並行書き込みが直列化され、更新が失われないこと。
#[tokio::test]
async fn concurrent_writes_do_not_lose_updates() {
    let db = connect().await;
    let name = unique_db("concurrent");

    let table = {
        let name = name.clone();
        db.write(async move |w| {
            w.database_create(&name, None).await?;
            w.table_create(&name, "t", TableDataType::Int, 25, None, None)
                .await
        })
        .await
        .unwrap()
    };

    // 同一テーブルへ別セルを並行に書き込む。テーブルスコープのロックを奪い合う。
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
                    TableDataType::Int,
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

    let table = {
        let name = name.clone();
        a.write(async move |w| {
            w.database_create(&name, None).await?;
            w.table_create(&name, "t", TableDataType::Int, 25, None, None)
                .await
        })
        .await
        .unwrap()
    };

    // 同じテーブルへ、両インスタンスから交互のセルを同時に書く。
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
                        TableDataType::Int,
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

    let users = a.read(async |r| r.get_all_users().await).await.unwrap();
    let roots = users.iter().filter(|u| u.username == "root").count();
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
                uuid::Uuid::now_v7(),
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
            w.table_create(&name, "t", TableDataType::Int, 25, None, None)
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

    let table = {
        let name = name.clone();
        db.write(async move |w| {
            w.database_create(&name, None).await?;
            w.table_create(&name, "t", TableDataType::Int, 25, None, None)
                .await
        })
        .await
        .unwrap()
    };

    let mut ids = SpatialIdSet::new();
    ids.insert(SingleId::new(20, 0, 7, 7).unwrap());

    let failed = db
        .write(async move |w| {
            w.data_insert(
                table.id,
                TableDataType::Int,
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
    assert_eq!(count, 0, "失敗した書き込みのセルが残っている");

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
                uuid::Uuid::now_v7(),
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
            let user = r.require_user(&username).await?;
            r.render_privileges(&user.privileges).await
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
