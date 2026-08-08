use heed::types::*;
use heed::{Database, Env, EnvFlags, EnvOpenOptions};

use crate::models::users::UserMetadata;
use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};
use crate::services::auth::hash_password;
use kasane_logic::FlexId;
use std::borrow::Cow;
use uuid::Uuid;
pub struct DbIdAndName;

impl<'a> heed::BytesEncode<'a> for DbIdAndName {
    type EItem = (crate::models::id::DatabaseId, &'a str);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(16 + item.1.len());
        bytes.extend_from_slice(&item.0.into_bytes());
        bytes.extend_from_slice(item.1.as_bytes());
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for DbIdAndName {
    type DItem = (crate::models::id::DatabaseId, &'a str);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() < 16 {
            return Err("invalid length".into());
        }
        let mut id = [0u8; 16];
        id.copy_from_slice(&bytes[0..16]);
        let id = crate::models::id::DatabaseId(uuid::Uuid::from_bytes(id));
        let name = std::str::from_utf8(&bytes[16..])?;
        Ok((id, name))
    }
}

pub struct TableIdAndFlexId;

impl<'a> heed::BytesEncode<'a> for TableIdAndFlexId {
    type EItem = (crate::models::id::TableId, FlexId);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(16 + FlexId::ENCODED_LEN);
        bytes.extend_from_slice(&item.0.into_bytes());
        bytes.extend_from_slice(&item.1.encode());
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for TableIdAndFlexId {
    type DItem = (crate::models::id::TableId, FlexId);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() != 16 + FlexId::ENCODED_LEN {
            return Err("invalid length for TableIdAndFlexId".into());
        }
        let mut table_id = [0u8; 16];
        table_id.copy_from_slice(&bytes[0..16]);
        let table_id = crate::models::id::TableId(uuid::Uuid::from_bytes(table_id));

        let mut flex_id_bytes = [0u8; FlexId::ENCODED_LEN];
        flex_id_bytes.copy_from_slice(&bytes[16..16 + FlexId::ENCODED_LEN]);
        let flex_id = FlexId::decode(&flex_id_bytes)?;

        Ok((table_id, flex_id))
    }
}

#[derive(Clone)]
pub struct AppDb {
    /// LMDBの環境本体
    pub env: Env<heed::WithoutTls>,

    /// データベースの一覧とメタデータを管理する
    /// Key: データベース名 (`Str`) -> Value: `DatabaseMetadata`
    pub databases: Database<Str, SerdeBincode<DatabaseMetadata>>,

    /// テーブルの一覧とメタデータを管理する
    /// Key: `DatabaseId` と テーブル名 (`DbIdAndName`) -> Value: `TableMetadata`
    pub tables: Database<DbIdAndName, SerdeJson<TableMetadata>>,

    /// `DatabaseId` からデータベース名を引くための逆引きインデックス。
    /// 権限は ID で保存されるため、それを名前へ戻すのに使う。
    /// Key: `DatabaseId` -> Value: データベース名 (`Str`)
    pub database_id_index: Database<SerdeBincode<crate::models::id::DatabaseId>, Str>,

    /// `TableId` の存在確認と、テーブル名の逆引きに用いるインデックス。
    /// Key: `TableId` -> Value: テーブル名 (`Str`)
    pub table_id_index: Database<SerdeBincode<crate::models::id::TableId>, Str>,

    /// 登録済みのユーザーを管理する
    /// Key: ユーザーID（現状は UUID などの文字列）(`Str`) -> Value: ユーザー名など (`Str`)
    pub users: Database<Str, Str>,

    /// FlexTree
    /// Key: `TableId` と 空間ID（`FlexId`）(`TableIdAndFlexId`) -> SpatialIdMapのバイト列(rkvy)
    pub tables_data: Database<TableIdAndFlexId, Bytes>,

    /// 値→空間の二次インデックス（値フィルタ用）。
    /// Key 生バイト列: `table_id(16) ‖ 順序保存エンコード値(可変) ‖ flexid.encode(FlexId::ENCODED_LEN)` -> 値なし
    pub value_index: Database<Bytes, Unit>,
}

#[tracing::instrument]
pub fn initialize_database(path: &str) -> AppDb {
    tracing::info!("Initializing database at: {}", path);
    std::fs::create_dir_all(path).unwrap();

    let map_size = std::env::var("KASANE_LMDB_MAP_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10 * 1024 * 1024 * 1024);

    let max_readers = std::env::var("KASANE_LMDB_MAX_READERS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1024);

    let env_bool = |name: &str| {
        std::env::var(name)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    };

    //LMDBの設定は全てデフォルト
    let mut env_flags = EnvFlags::empty();
    if env_bool("KASANE_LMDB_NO_READAHEAD") {
        env_flags |= EnvFlags::NO_READ_AHEAD;
    }
    let write_map = env_bool("KASANE_LMDB_WRITE_MAP");
    if write_map {
        env_flags |= EnvFlags::WRITE_MAP;
    }
    if env_bool("KASANE_LMDB_NO_SYNC") {
        env_flags |= EnvFlags::NO_SYNC | EnvFlags::NO_META_SYNC;
        if write_map {
            env_flags |= EnvFlags::MAP_ASYNC;
        }
    }

    let env = unsafe {
        let mut opts = EnvOpenOptions::new().read_txn_without_tls();
        opts.map_size(map_size).max_dbs(15).max_readers(max_readers);
        if !env_flags.is_empty() {
            opts.flags(env_flags);
        }
        opts.open(path).unwrap_or_else(|e| {
            tracing::error!("Failed to open heed Env at {}: {}", path, e);
            panic!("Failed to open heed Env: {}", e);
        })
    };

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

    if users.is_empty(&write_txn).unwrap() {
        let default_username = "root";
        let default_password = std::env::var("ROOT_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "password".to_string());

        tracing::info!("Creating default root user: {}", default_username);
        let hash = hash_password(&default_password).unwrap();
        let root_meta = UserMetadata {
            id: Uuid::now_v7(),
            password_hash: hash,
            token_version: 0,
            privileges: vec![crate::models::users::StoredPrivilege::Global {
                role: crate::models::users::UserRole::Admin,
            }],
        };
        let json = serde_json::to_string(&root_meta).unwrap();
        users
            .put(&mut write_txn, default_username, json.as_str())
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
