//! 起動時に一度だけ行う設定をここに集める。

use heed::{Env, EnvFlags, EnvOpenOptions};
use uuid::Uuid;

use crate::models::id::PrincipalId;
use crate::models::users::{UserRecord, UserRole};
use crate::repositories::{ROOT_USERNAME, SCHEMA_VERSION, root_password};
use crate::services::auth::hash_password;

use super::AppDb;

const SCHEMA_VERSION_KEY: &str = "schema_version";

/// マップサイズの既定値（10 GiB）。`KASANE_LMDB_MAP_SIZE` で上書きできる。
const DEFAULT_MAP_SIZE: usize = 10 * 1024 * 1024 * 1024;
/// 同時リーダ数の既定値。`KASANE_LMDB_MAX_READERS` で上書きできる。
const DEFAULT_MAX_READERS: u32 = 1024;
/// 開くサブデータベース数の上限。`AppDb` のフィールド数より十分大きく取る。
const MAX_DBS: u32 = 24;

fn env_bool(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// 環境変数を数値として読む。未設定・解析不能なら既定値。
fn env_parsed<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// 同期を緩めるフラグはクラッシュ時の耐久性と引き換えなので、既定では立てない。
fn env_flags() -> EnvFlags {
    let mut flags = EnvFlags::empty();
    if env_bool("KASANE_LMDB_NO_READAHEAD") {
        flags |= EnvFlags::NO_READ_AHEAD;
    }
    let write_map = env_bool("KASANE_LMDB_WRITE_MAP");
    if write_map {
        flags |= EnvFlags::WRITE_MAP;
    }
    if env_bool("KASANE_LMDB_NO_SYNC") {
        flags |= EnvFlags::NO_SYNC | EnvFlags::NO_META_SYNC;
        if write_map {
            flags |= EnvFlags::MAP_ASYNC;
        }
    }
    flags
}

fn open_env(path: &str) -> Env<heed::WithoutTls> {
    let flags = env_flags();
    unsafe {
        let mut opts = EnvOpenOptions::new().read_txn_without_tls();
        opts.map_size(env_parsed("KASANE_LMDB_MAP_SIZE", DEFAULT_MAP_SIZE))
            .max_dbs(MAX_DBS)
            .max_readers(env_parsed("KASANE_LMDB_MAX_READERS", DEFAULT_MAX_READERS));
        if !flags.is_empty() {
            opts.flags(flags);
        }
        opts.open(path).unwrap_or_else(|e| {
            tracing::error!("Failed to open heed Env at {}: {}", path, e);
            panic!("Failed to open heed Env: {}", e);
        })
    }
}

#[tracing::instrument]
pub fn initialize_database(path: &str) -> AppDb {
    tracing::info!("Initializing database at: {}", path);
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
    let value_index = env
        .create_database(&mut write_txn, Some("value_index"))
        .unwrap();

    // 版の確認は他のどの書き込みより先に行う。読み方が違うまま触ると壊れる。
    let stored = meta.get(&write_txn, SCHEMA_VERSION_KEY).unwrap();
    let is_empty = users.is_empty(&write_txn).unwrap();
    match stored {
        Some(SCHEMA_VERSION) => {}
        Some(other) => panic!(
            "このディレクトリは Kasane のディスク形式 v{other} で書かれています\
             （このビルドが読めるのは v{SCHEMA_VERSION}）。\
             別のディレクトリを指定するか、DATABASE_DIR を新しい場所へ向けてください: {path}"
        ),
        // 版を持たないが中身はある = 権限を利用者レコードに埋めていた頃の形式。
        None if !is_empty => panic!(
            "このディレクトリは Kasane の古いディスク形式（v1、権限が利用者レコードに\
             同居していた頃）で書かれています。このビルドが読めるのは v{SCHEMA_VERSION} です。\
             別のディレクトリを指定するか、DATABASE_DIR を新しい場所へ向けてください: {path}"
        ),
        None => {
            meta.put(&mut write_txn, SCHEMA_VERSION_KEY, &SCHEMA_VERSION)
                .unwrap();
        }
    }

    // 単一ライタなので、ここでの「空なら作る」は他プロセスと競合しない。
    if is_empty {
        tracing::info!("Creating default root user: {}", ROOT_USERNAME);
        let json = serde_json::to_string(&root_user_record()).unwrap();
        users
            .put(&mut write_txn, ROOT_USERNAME, json.as_str())
            .unwrap();
    }

    write_txn.commit().unwrap();
    tracing::info!("Database initialized successfully (schema v{SCHEMA_VERSION})");

    AppDb {
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
        value_index,
    }
}

/// root は `global` / `admin` だけを持つ。ACL 行は 1 つも要らない。
fn root_user_record() -> UserRecord {
    UserRecord {
        id: PrincipalId(Uuid::now_v7()),
        password_hash: hash_password(&root_password()).unwrap(),
        token_version: 0,
        global_role: Some(UserRole::Admin),
    }
}
