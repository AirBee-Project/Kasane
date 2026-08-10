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

/// 起動時の root ユーザー作成が、複数インスタンス同時起動でも二重にならないこと。
#[tokio::test]
async fn root_user_is_seeded_exactly_once() {
    // 同時接続。どちらも `connect` の中で root の有無を見て作ろうとする。
    let (a, b) = tokio::join!(connect(), connect());

    let users = a.read(async |r| r.get_all_users().await).await.unwrap();
    let roots = users.iter().filter(|u| u.username == "root").count();
    assert_eq!(roots, 1, "root ユーザーが重複して作られている");

    // どちらのインスタンスからも同じ root が見えること。
    let from_b = b.read(async |r| r.get_user("root").await).await.unwrap();
    assert!(from_b.is_some(), "別インスタンスから root が見えない");
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
