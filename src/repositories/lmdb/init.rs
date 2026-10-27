//! 起動時に一度だけ行う設定をここに集める。

use std::path::Path;

use heed::{Env, EnvOpenOptions};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::id::PrincipalId;
use crate::models::users::{UserRecord, UserRole};
use crate::repositories::{ROOT_USERNAME, SCHEMA_VERSION, root_password};
use crate::services::auth::hash_password;

use super::Lmdb;

const SCHEMA_VERSION_KEY: &str = "schema_version";

const MAP_SIZE: usize = 10 * 1024 * 1024 * 1024;
const MAX_READERS: u32 = 1024;
/// 開くサブデータベース数の上限。`Lmdb` のフィールド数より十分大きく取る。
const MAX_DBS: u32 = 24;

fn open_env(path: &Path) -> Env<heed::WithoutTls> {
    unsafe {
        EnvOpenOptions::new()
            .read_txn_without_tls()
            .map_size(MAP_SIZE)
            .max_dbs(MAX_DBS)
            .max_readers(MAX_READERS)
            .open(path)
            .unwrap_or_else(|e| {
                tracing::error!("Failed to open heed Env at {}: {}", path.display(), e);
                panic!("Failed to open heed Env: {}", e);
            })
    }
}

/// 版が合わなければエラーを返す。
#[tracing::instrument(skip_all)]
pub fn initialize_database(path: impl AsRef<Path>) -> Result<Lmdb, AppError> {
    let path = path.as_ref();
    tracing::info!("Initializing database at: {}", path.display());
    std::fs::create_dir_all(path).unwrap();

    let env = open_env(path);
    let mut write_txn = env.write_txn().unwrap();

    let meta = env.create_database(&mut write_txn, Some("meta")).unwrap();
    let databases = env
        .create_database(&mut write_txn, Some("databases"))
        .unwrap();
    let tables = env.create_database(&mut write_txn, Some("tables")).unwrap();
    let database_id_index = env
        .create_database(&mut write_txn, Some("database_id_index"))
        .unwrap();
    let table_id_index = env
        .create_database(&mut write_txn, Some("table_id_index"))
        .unwrap();
    let users = env.create_database(&mut write_txn, Some("users")).unwrap();
    let acl = env.create_database(&mut write_txn, Some("acl")).unwrap();
    let acl_by_object = env
        .create_database(&mut write_txn, Some("acl_by_object"))
        .unwrap();
    let tables_data = env
        .create_database(&mut write_txn, Some("tables_data"))
        .unwrap();

    // 版の確認は他のどの書き込みより先に行う。読み方が違うまま触ると壊れる。
    let found = meta.get(&write_txn, SCHEMA_VERSION_KEY)?;
    let is_empty = users.is_empty(&write_txn)?;
    match found {
        Some(SCHEMA_VERSION) => {}
        // 版が違う、あるいは版を持たないのに中身がある（版を刻む前の世代）。
        Some(_) => return Err(mismatch(found)),
        None if !is_empty => return Err(mismatch(found)),
        None => meta.put(&mut write_txn, SCHEMA_VERSION_KEY, &SCHEMA_VERSION)?,
    }

    // 単一ライタなので、ここでの「空なら作る」は他プロセスと競合しない。
    if is_empty {
        tracing::info!("Creating default root user: {ROOT_USERNAME}");
        let record = root_user_record()?;
        let json = serde_json::to_string(&record)
            .map_err(|e| AppError::corrupt(crate::error::Stored::UserRecord, e))?;
        users.put(&mut write_txn, ROOT_USERNAME, json.as_str())?;
    }

    write_txn.commit()?;
    tracing::info!("Database initialized successfully (schema v{SCHEMA_VERSION})");

    Ok(Lmdb {
        env,
        meta,
        databases,
        tables,
        database_id_index,
        table_id_index,
        users,
        acl,
        acl_by_object,
        tables_data,
    })
}

fn mismatch(found: Option<u32>) -> AppError {
    let found = match found {
        Some(found) => format!("schema version {found}"),
        None => "data without a schema version".to_string(),
    };
    AppError::Internal(format!(
        "On-disk {found} is not supported by this build (it reads version {SCHEMA_VERSION}); \
         point the server at a fresh location"
    ))
}

/// root は `global` / `admin` だけを持つ。ACL 行は 1 つも要らない。
fn root_user_record() -> Result<UserRecord, AppError> {
    Ok(UserRecord {
        id: PrincipalId(Uuid::now_v7()),
        password_hash: hash_password(&root_password())?,
        token_version: 0,
        global_role: Some(UserRole::Admin),
    })
}
