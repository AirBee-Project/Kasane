//! 削除の後始末のように、API からは観測できないストレージの中身を確かめるテスト。
//!
//! 結合テスト（`tests/`）からはストレージに触れないので、クレートの中に置いて `Kasane` のメソッドを直接呼ぶ。

use tempfile::TempDir;

use crate::Kasane;
use crate::error::AppError;
use crate::models::table::{CreateTableRequest, TableDataType};
use crate::models::users::{
    CreateUserRequest, DataRole, MAX_PRIVILEGES_PER_USER, PrivilegeRule, User,
};
use crate::repositories::{CatalogRepository, ROOT_USERNAME, Storage};

/// `TempDir` は LMDB を開いている間だけ残す必要があるので、一緒に返す。
async fn open() -> (Kasane, User, TempDir) {
    let dir = TempDir::new().unwrap();
    let kasane = Kasane::open(dir.path()).unwrap();
    let root = kasane.user(ROOT_USERNAME).await.unwrap();
    (kasane, root, dir)
}

async fn create_table(kasane: &Kasane, root: &User, db_name: &str, table_name: &str) {
    let req = CreateTableRequest {
        name: table_name.to_string(),
        data_type: TableDataType::Int,
        max_zoom_level: 5,
        constraints: None,
        description: None,
        is_temporal: false,
    };
    kasane.create_table(root, db_name, req).await.unwrap();
}

/// `GetUser` は解決できない行を隠すので、残留は保存されている ACL 行を直接数えて確かめる。
async fn stored_privilege_count(kasane: &Kasane, username: &str) -> usize {
    let username = username.to_string();
    kasane
        .db
        .read(async move |r| {
            let record = r.require_user_record(&username).await?;
            Ok(r.acl_entries(record.id).await?.len())
        })
        .await
        .unwrap()
}

/// `table_id_index` に残っているテーブルの総数。到達不能になったテーブルの検出に使う。
fn indexed_table_count(kasane: &Kasane) -> usize {
    let rtxn = kasane.db.env.read_txn().unwrap();
    kasane.db.table_id_index.len(&rtxn).unwrap() as usize
}

/// 権限は対象ごとの行として持ち、対象を消すときに逆引き索引からその行を落とす。
/// よって「作成 → 付与 → 削除」を何度繰り返しても、残留は**常に 0 件**になる。
#[tokio::test]
async fn stale_privileges_do_not_accumulate_over_cycles() {
    let (kasane, root, _dir) = open().await;
    kasane
        .create_database(&root, "churn_db", None)
        .await
        .unwrap();
    kasane
        .create_user(
            &root,
            CreateUserRequest {
                username: "churner".to_string(),
                password: "password".to_string(),
                privileges: Vec::new(),
            },
        )
        .await
        .unwrap();

    for i in 0..10 {
        let table_name = format!("tmp_{i}");
        create_table(&kasane, &root, "churn_db", &table_name).await;
        let rule = PrivilegeRule::Table {
            db_name: "churn_db".to_string(),
            table_name: table_name.clone(),
            role: DataRole::Read,
        };
        kasane
            .grant_privilege(&root, "churner", rule)
            .await
            .unwrap();
        kasane
            .remove_table(&root, "churn_db", &table_name)
            .await
            .unwrap();

        assert_eq!(
            stored_privilege_count(&kasane, "churner").await,
            0,
            "サイクル {i} で削除済みテーブルを指す行が残った"
        );
    }

    let detail = kasane.user_detail(&root, "churner").await.unwrap();
    assert!(detail.privileges.is_empty());
}

/// 列挙と削除が別トランザクションに分かれていると、その隙間に作られたテーブルが
/// 親を失って到達不能なまま残る。ここでは削除の完全性そのものを固定する。
#[tokio::test]
async fn database_remove_leaves_no_orphan_tables() {
    let (kasane, root, _dir) = open().await;
    kasane
        .create_database(&root, "doomed_db", None)
        .await
        .unwrap();
    kasane
        .create_database(&root, "kept_db", None)
        .await
        .unwrap();
    for i in 0..3 {
        create_table(&kasane, &root, "doomed_db", &format!("t_{i}")).await;
    }
    create_table(&kasane, &root, "kept_db", "survivor").await;
    assert_eq!(indexed_table_count(&kasane), 4);

    kasane.remove_database(&root, "doomed_db").await.unwrap();
    assert_eq!(indexed_table_count(&kasane), 1);

    // 同名で作り直しても、以前のテーブルは見えない。
    kasane
        .create_database(&root, "doomed_db", None)
        .await
        .unwrap();
    let tables = kasane.tables(&root, "doomed_db").await.unwrap();
    assert!(tables.is_empty());
}

/// gRPC 越しだと、上限ぶんのルールを送った時点でメッセージサイズの上限に先に当たり、
/// 件数チェックまで届かない。そのためリポジトリ層で確かめる。
#[tokio::test]
async fn privilege_rules_are_capped_before_resolution() {
    let (kasane, _root, _dir) = open().await;

    // 名前解決が先に走るなら database_not_found になるはずだが、件数チェックが先なので
    // InvalidArgument で落ちる。
    let rules: Vec<PrivilegeRule> = (0..MAX_PRIVILEGES_PER_USER as usize + 1)
        .map(|i| PrivilegeRule::Database {
            db_name: format!("ghost_{i}"),
            role: DataRole::Read,
        })
        .collect();

    let err = kasane
        .db
        .read(async move |r| r.resolve_rules(&rules).await)
        .await
        .expect_err("上限超えが受理された");

    assert!(
        matches!(err, AppError::InvalidArgument { .. }),
        "件数ではなく名前解決で落ちている: {err:?}"
    );
}
