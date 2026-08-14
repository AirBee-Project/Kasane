//! [`Storage`] trait 経由でのトランザクション境界が実際に機能することの検証。
//!
//! クロージャ方式の非同期 API が、実 LMDB に対して読み書き・コミット・
//! エラー時の破棄まで期待どおりに振る舞うことを確かめる。
//! TiKV バックエンドのビルドでは対象外（同等の検証は `tikv_backend.rs`）。
#![cfg(feature = "backend-lmdb")]

use kasane::error::AppError;
use kasane::repositories::lmdb::initialize_database;
use kasane::repositories::{ReadRepository, Storage, WriteRepository};

fn temp_db() -> (tempfile::TempDir, kasane::repositories::lmdb::AppDb) {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap()).unwrap();
    (tmp, db)
}

#[tokio::test]
async fn write_then_read_through_trait() {
    let (_tmp, db) = temp_db();

    let created = Storage::write(&db, async |w| {
        WriteRepository::database_create(w, "trait_db", Some("via trait".into())).await
    })
    .await
    .unwrap();
    assert_eq!(created.name, "trait_db");

    // 別トランザクションから読めること（＝コミットされていること）。
    let info = Storage::read(&db, async |r| {
        ReadRepository::database_info(r, "trait_db").await
    })
    .await
    .unwrap()
    .expect("committed database should be visible");
    assert_eq!(info.1.description.as_deref(), Some("via trait"));
}

#[tokio::test]
async fn error_in_closure_discards_the_transaction() {
    let (_tmp, db) = temp_db();

    // 作成したうえでエラーを返す → コミットされないこと。
    let result: Result<(), AppError> = Storage::write(&db, async |w| {
        WriteRepository::database_create(w, "rolled_back", None).await?;
        Err(AppError::InternalError("boom".into()))
    })
    .await;
    assert!(result.is_err());

    let info = Storage::read(&db, async |r| {
        ReadRepository::database_info(r, "rolled_back").await
    })
    .await
    .unwrap();
    assert!(
        info.is_none(),
        "closure が失敗したトランザクションはコミットされてはいけない"
    );
}

#[tokio::test]
async fn write_closure_sees_its_own_writes() {
    let (_tmp, db) = temp_db();

    // 同一トランザクション内で、書いた直後に自分で読めること
    // （既存の重複チェックなどがこの性質に依存している）。
    let seen = Storage::write(&db, async |w| {
        WriteRepository::database_create(w, "ryw", None).await?;
        WriteRepository::database_info(w, "ryw").await
    })
    .await
    .unwrap();
    assert!(
        seen.is_some(),
        "自分の書き込みが同一トランザクションから見えていない"
    );
}

#[tokio::test]
async fn meta_repository_defaults_work_over_async_lookups() {
    use kasane::models::users::{AclEntry, DataRole, PrivilegeRule, ResolvedPrivilege};
    use kasane::repositories::CatalogRepository;

    let (_tmp, db) = temp_db();

    Storage::write(&db, async |w| {
        WriteRepository::database_create(w, "perm_db", None).await
    })
    .await
    .unwrap();

    // 名前 → ID の解決と、その逆の描画が既定実装を通して往復すること。
    let rendered = Storage::read(&db, async |r| {
        let resolved = CatalogRepository::resolve_rules(
            r,
            &[PrivilegeRule::Database {
                db_name: "perm_db".into(),
                role: DataRole::Read,
            }],
        )
        .await?;

        // 解決した対象を ACL 行の形へ移してから描画する（保存されるのはこの形）。
        let entries: Vec<AclEntry> = resolved
            .into_iter()
            .map(|resolved| match resolved {
                ResolvedPrivilege::Data { target, role } => AclEntry { target, role },
                other => panic!("想定外の対象: {other:?}"),
            })
            .collect();
        CatalogRepository::render_privileges(r, None, &entries).await
    })
    .await
    .unwrap();

    assert_eq!(
        rendered,
        vec![PrivilegeRule::Database {
            db_name: "perm_db".into(),
            role: DataRole::Read,
        }]
    );
}

#[path = "shared/acl_suite.rs"]
mod acl_suite;

/// ACL 行の保存が仕様どおりか（TiKV 側は `tikv.rs` が同じスイートを回す）。
#[tokio::test]
async fn acl_rows_behave_as_specified() {
    let (_tmp, db) = temp_db();
    acl_suite::run(&db, "lmdb").await;
}

#[tokio::test]
async fn acl_driven_listing_behaves_as_specified() {
    let (_tmp, db) = temp_db();
    acl_suite::run_listing(&db, "lmdblist").await;
}
