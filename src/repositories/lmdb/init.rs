//! 起動時に一度だけ行う設定をここに集める。

use heed::{Env, EnvFlags, EnvOpenOptions};
use uuid::Uuid;

use crate::models::users::{StoredPrivilege, UserMetadata, UserRole};
use crate::services::auth::hash_password;

use super::AppDb;

/// マップサイズの既定値（10 GiB）。`KASANE_LMDB_MAP_SIZE` で上書きできる。
const DEFAULT_MAP_SIZE: usize = 10 * 1024 * 1024 * 1024;
/// 同時リーダ数の既定値。`KASANE_LMDB_MAX_READERS` で上書きできる。
const DEFAULT_MAX_READERS: u32 = 1024;
/// 開くサブデータベース数の上限。`AppDb` のフィールド数より十分大きく取る。
const MAX_DBS: u32 = 15;

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
    let tables_data = env
        .create_database(&mut write_txn, Some("tables_data"))
        .unwrap();
    let value_index = env
        .create_database(&mut write_txn, Some("value_index"))
        .unwrap();

    // 単一ライタなので、ここでの「空なら作る」は他プロセスと競合しない。
    if users.is_empty(&write_txn).unwrap() {
        let root = root_user_metadata();
        tracing::info!("Creating default root user: {}", ROOT_USERNAME);
        let json = serde_json::to_string(&root).unwrap();
        users
            .put(&mut write_txn, ROOT_USERNAME, json.as_str())
            .unwrap();
    }

    write_txn.commit().unwrap();
    tracing::info!("Database initialized successfully");

    AppDb {
        env,
        databases,
        tables,
        database_id_index,
        table_id_index,
        users,
        tables_data,
        value_index,
    }
}

const ROOT_USERNAME: &str = "root";

fn root_user_metadata() -> UserMetadata {
    let password = std::env::var("ROOT_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "password".to_string());

    UserMetadata {
        id: Uuid::now_v7(),
        password_hash: hash_password(&password).unwrap(),
        token_version: 0,
        privileges: vec![StoredPrivilege::Global {
            role: UserRole::Admin,
        }],
    }
}
