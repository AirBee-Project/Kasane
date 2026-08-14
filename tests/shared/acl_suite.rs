//! 両バックエンドが同じ ACL の振る舞いをすることの検証。
//!
//! 認可の**規則**（どのロールがどれを含むか、どのスコープがどこまで届くか）は
//! `User::allows` のユニットテストが持つ。ここで確かめるのは**行の保存**のほう――
//! スコープごとに正しい行が引けるか、対象を消したときに行が消えるか、といった
//! バックエンドごとに実装が分かれる部分。
//!
//! `#[path]` 付きの `mod` で各テストバイナリへ取り込む。1 つのクラスタを共有する
//! バックエンドでも並行に走れるよう、名前はすべて `tag` から作る。

#![allow(dead_code)]

use kasane::error::AppError;
use kasane::models::database::table::TableDataType;
use kasane::models::users::{DataRole, Grant, PrivilegeRule, PrivilegeTarget, Scope, UserRole};
use kasane::repositories::{CatalogRepository, ReadRepository, Storage, WriteRepository};

/// このスイートが作る名前。`tag` はテストごとに一意にすること。
struct Names {
    db: String,
    other_db: String,
    table: String,
    other_table: String,
    user: String,
}

impl Names {
    fn new(tag: &str) -> Self {
        Self {
            db: format!("acl_{tag}_db"),
            other_db: format!("acl_{tag}_odb"),
            table: format!("acl_{tag}_t"),
            other_table: format!("acl_{tag}_ot"),
            user: format!("acl_{tag}_u"),
        }
    }
}

async fn principal<S: Storage>(db: &S, username: &str) -> kasane::models::id::PrincipalId {
    let username = username.to_string();
    db.read(async move |r| Ok(r.require_user_record(&username).await?.id))
        .await
        .unwrap()
}

async fn grant_of<S: Storage>(db: &S, username: &str, scope: Scope) -> Grant {
    let id = principal(db, username).await;
    db.read(async move |r| r.grant_for(id, scope).await)
        .await
        .unwrap()
}

async fn entry_count<S: Storage>(db: &S, username: &str) -> usize {
    let id = principal(db, username).await;
    db.read(async move |r| Ok(r.acl_entries(id).await?.len()))
        .await
        .unwrap()
}

async fn rendered<S: Storage>(db: &S, username: &str) -> Vec<PrivilegeRule> {
    let username = username.to_string();
    db.read(async move |r| {
        let record = r.require_user_record(&username).await?;
        let entries = r.acl_entries(record.id).await?;
        r.render_privileges(record.global_role, &entries).await
    })
    .await
    .unwrap()
}

async fn setup<S: Storage>(db: &S, n: &Names) {
    let (d, o, t, ot) = (
        n.db.clone(),
        n.other_db.clone(),
        n.table.clone(),
        n.other_table.clone(),
    );
    db.write(async move |w| {
        w.database_create(&d, None).await?;
        w.database_create(&o, None).await?;
        w.table_create(&d, &t, TableDataType::Int, 20, None, None, false)
            .await?;
        w.table_create(&d, &ot, TableDataType::Int, 20, None, None, false)
            .await?;
        Ok(())
    })
    .await
    .unwrap();

    let user = n.user.clone();
    db.write(async move |w| {
        w.create_user(
            &user,
            kasane::models::id::PrincipalId(uuid::Uuid::now_v7()),
            "hash".to_string(),
            &[],
        )
        .await
    })
    .await
    .unwrap();
}

async fn teardown<S: Storage>(db: &S, n: &Names) {
    let user = n.user.clone();
    let _ = db.write(async move |w| w.delete_user(&user).await).await;
    for name in [n.db.clone(), n.other_db.clone()] {
        let _ = db
            .write(async move |w| w.database_remove(&name).await)
            .await;
    }
}

async fn table_id<S: Storage>(
    db: &S,
    db_name: &str,
    table_name: &str,
) -> kasane::models::id::TableId {
    let (d, t) = (db_name.to_string(), table_name.to_string());
    db.read(async move |r| {
        let db_id = r.require_database_id(&d).await?;
        Ok(r.table_id(db_id, &t).await?.expect("テーブルが無い"))
    })
    .await
    .unwrap()
}

async fn database_id<S: Storage>(db: &S, db_name: &str) -> kasane::models::id::DatabaseId {
    let d = db_name.to_string();
    db.read(async move |r| r.require_database_id(&d).await)
        .await
        .unwrap()
}

async fn grant<S: Storage>(db: &S, username: &str, rule: PrivilegeRule) -> Result<(), AppError> {
    let username = username.to_string();
    db.write(async move |w| w.grant_privilege(&username, &rule).await)
        .await
}

async fn revoke<S: Storage>(
    db: &S,
    username: &str,
    target: PrivilegeTarget,
) -> Result<(), AppError> {
    let username = username.to_string();
    db.write(async move |w| w.revoke_privilege(&username, &target).await)
        .await
}

/// スイート本体。`tag` はテストごとに一意な短い識別子。
pub async fn run<S: Storage>(db: &S, tag: &str) {
    let n = Names::new(tag);
    teardown(db, &n).await;
    setup(db, &n).await;

    let db_id = database_id(db, &n.db).await;
    let t_id = table_id(db, &n.db, &n.table).await;
    let ot_id = table_id(db, &n.db, &n.other_table).await;
    let other_db_id = database_id(db, &n.other_db).await;

    // --- 権限を持たない状態 ---
    assert_eq!(
        grant_of(db, &n.user, Scope::Database(db_id)).await,
        Grant::Database(None)
    );
    assert_eq!(
        grant_of(db, &n.user, Scope::AnyIn(db_id)).await,
        Grant::AnyIn(false)
    );

    // --- テーブル単位の付与 ---
    grant(
        db,
        &n.user,
        PrivilegeRule::Table {
            db_name: n.db.clone(),
            table_name: n.table.clone(),
            role: DataRole::Manage,
        },
    )
    .await
    .unwrap();

    // そのテーブルには届く。
    assert_eq!(
        grant_of(db, &n.user, Scope::Table(db_id, t_id)).await,
        Grant::Table {
            database: None,
            table: Some(DataRole::Manage),
        }
    );
    // 同じデータベースの別テーブルには届かない。
    assert_eq!(
        grant_of(db, &n.user, Scope::Table(db_id, ot_id)).await,
        Grant::Table {
            database: None,
            table: None,
        }
    );
    // データベース全体の操作には効かない。
    assert_eq!(
        grant_of(db, &n.user, Scope::Database(db_id)).await,
        Grant::Database(None)
    );
    // 配下に 1 行あるので「見える」。
    assert_eq!(
        grant_of(db, &n.user, Scope::AnyIn(db_id)).await,
        Grant::AnyIn(true)
    );
    // 別のデータベースには波及しない。
    assert_eq!(
        grant_of(db, &n.user, Scope::AnyIn(other_db_id)).await,
        Grant::AnyIn(false)
    );

    // --- データベース単位の付与を重ねる ---
    grant(
        db,
        &n.user,
        PrivilegeRule::Database {
            db_name: n.db.clone(),
            role: DataRole::Read,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        grant_of(db, &n.user, Scope::Table(db_id, t_id)).await,
        Grant::Table {
            database: Some(DataRole::Read),
            table: Some(DataRole::Manage),
        }
    );
    assert_eq!(
        grant_of(db, &n.user, Scope::Database(db_id)).await,
        Grant::Database(Some(DataRole::Read))
    );
    assert_eq!(entry_count(db, &n.user).await, 2);

    // --- 同じ対象への再付与は置き換え（増えない） ---
    grant(
        db,
        &n.user,
        PrivilegeRule::Database {
            db_name: n.db.clone(),
            role: DataRole::Write,
        },
    )
    .await
    .unwrap();
    assert_eq!(entry_count(db, &n.user).await, 2);
    assert_eq!(
        grant_of(db, &n.user, Scope::Database(db_id)).await,
        Grant::Database(Some(DataRole::Write))
    );

    // --- 描画（名前へ戻す） ---
    let mut names: Vec<String> = rendered(db, &n.user)
        .await
        .into_iter()
        .map(|rule| match rule {
            PrivilegeRule::Global { .. } => "global".to_string(),
            PrivilegeRule::Database { db_name, .. } => db_name,
            PrivilegeRule::Table { table_name, .. } => table_name,
        })
        .collect();
    names.sort();
    let mut want = vec![n.db.clone(), n.table.clone()];
    want.sort();
    assert_eq!(names, want, "描画された権限が保存内容と一致しない");

    // --- 全体ロールは行にせず利用者レコードへ ---
    grant(
        db,
        &n.user,
        PrivilegeRule::Global {
            role: UserRole::Read,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        entry_count(db, &n.user).await,
        2,
        "global スコープが ACL 行として保存されている"
    );
    let user = n.user.clone();
    let global = db
        .read(async move |r| Ok(r.require_user_record(&user).await?.global_role))
        .await
        .unwrap();
    assert_eq!(global, Some(UserRole::Read));

    // --- 剥奪は対象ごと、ロールは問わない ---
    revoke(
        db,
        &n.user,
        PrivilegeTarget::Database {
            db_name: n.db.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(entry_count(db, &n.user).await, 1);
    assert_eq!(
        grant_of(db, &n.user, Scope::Database(db_id)).await,
        Grant::Database(None)
    );

    // 持っていない対象の剥奪は NotFound。
    let err = revoke(
        db,
        &n.user,
        PrivilegeTarget::Database {
            db_name: n.other_db.clone(),
        },
    )
    .await
    .expect_err("持っていない権限の剥奪が成功した");
    assert!(
        matches!(err, AppError::NotFound(_)),
        "想定外の失敗: {err:?}"
    );

    // --- 対象の削除が行を落とす ---
    assert_eq!(
        grant_of(db, &n.user, Scope::AnyIn(db_id)).await,
        Grant::AnyIn(true)
    );
    let (d, t) = (n.db.clone(), n.table.clone());
    db.write(async move |w| w.table_remove(&d, &t).await)
        .await
        .unwrap();

    assert_eq!(
        entry_count(db, &n.user).await,
        0,
        "削除したテーブルを指す ACL 行が残っている"
    );
    assert_eq!(
        grant_of(db, &n.user, Scope::AnyIn(db_id)).await,
        Grant::AnyIn(false),
        "削除したテーブルの行でデータベースが見え続けている"
    );

    // --- データベースの削除も同様（スコープ行と配下の行の両方） ---
    grant(
        db,
        &n.user,
        PrivilegeRule::Database {
            db_name: n.db.clone(),
            role: DataRole::Read,
        },
    )
    .await
    .unwrap();
    grant(
        db,
        &n.user,
        PrivilegeRule::Table {
            db_name: n.db.clone(),
            table_name: n.other_table.clone(),
            role: DataRole::Read,
        },
    )
    .await
    .unwrap();
    assert_eq!(entry_count(db, &n.user).await, 2);

    let d = n.db.clone();
    db.write(async move |w| w.database_remove(&d).await)
        .await
        .unwrap();
    assert_eq!(
        entry_count(db, &n.user).await,
        0,
        "削除したデータベースを指す ACL 行が残っている"
    );

    // --- 利用者の削除は自分の行をすべて落とす ---
    grant(
        db,
        &n.user,
        PrivilegeRule::Database {
            db_name: n.other_db.clone(),
            role: DataRole::Read,
        },
    )
    .await
    .unwrap();
    let id = principal(db, &n.user).await;
    assert_eq!(entry_count(db, &n.user).await, 1);

    let user = n.user.clone();
    db.write(async move |w| w.delete_user(&user).await)
        .await
        .unwrap();
    let left = db
        .read(async move |r| Ok(r.acl_entries(id).await?.len()))
        .await
        .unwrap();
    assert_eq!(left, 0, "削除した利用者の ACL 行が残っている");

    teardown(db, &n).await;
}

/// 権限の側から一覧を引く経路（`acl_databases` / `acl_tables_in`）。
pub async fn run_listing<S: Storage>(db: &S, tag: &str) {
    let n = Names::new(tag);
    teardown(db, &n).await;
    setup(db, &n).await;

    let db_id = database_id(db, &n.db).await;
    let t_id = table_id(db, &n.db, &n.table).await;

    grant(
        db,
        &n.user,
        PrivilegeRule::Table {
            db_name: n.db.clone(),
            table_name: n.table.clone(),
            role: DataRole::Read,
        },
    )
    .await
    .unwrap();

    let id = principal(db, &n.user).await;

    // 権限を持つデータベースだけが挙がる。
    let dbs = db
        .read(async move |r| r.acl_databases(id).await)
        .await
        .unwrap();
    assert!(dbs.contains(&db_id));
    assert_eq!(dbs.len(), 1, "権限の無いデータベースまで挙がっている");

    // 配下はテーブル単位の行だけが挙がる。
    let tables = db
        .read(async move |r| r.acl_tables_in(id, db_id).await)
        .await
        .unwrap();
    assert_eq!(tables.iter().copied().collect::<Vec<_>>(), vec![t_id]);

    // ID から本体を引き直せる。
    let ids: Vec<_> = dbs.into_iter().collect();
    let listed = db
        .read(async move |r| r.databases_by_id(&ids).await)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1.name, n.db);

    let table_ids = vec![t_id];
    let listed = db
        .read(async move |r| r.tables_by_id(db_id, &table_ids).await)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, n.table);

    // データベーススコープの行は `acl_tables_in` に混ざらない。
    grant(
        db,
        &n.user,
        PrivilegeRule::Database {
            db_name: n.db.clone(),
            role: DataRole::Read,
        },
    )
    .await
    .unwrap();
    let tables = db
        .read(async move |r| r.acl_tables_in(id, db_id).await)
        .await
        .unwrap();
    assert_eq!(
        tables.len(),
        1,
        "データベーススコープの行がテーブル一覧へ混ざっている"
    );

    teardown(db, &n).await;
}
