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

pub struct UserIdAndDbId;

impl<'a> heed::BytesEncode<'a> for UserIdAndDbId {
    type EItem = (crate::models::id::UserId, crate::models::id::DatabaseId);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&item.0.into_bytes());
        bytes.extend_from_slice(&item.1.into_bytes());
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for UserIdAndDbId {
    type DItem = (crate::models::id::UserId, crate::models::id::DatabaseId);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() != 32 {
            return Err("invalid length".into());
        }
        let mut uid = [0u8; 16];
        uid.copy_from_slice(&bytes[0..16]);
        let mut dbid = [0u8; 16];
        dbid.copy_from_slice(&bytes[16..32]);
        Ok((
            crate::models::id::UserId(uuid::Uuid::from_bytes(uid)),
            crate::models::id::DatabaseId(uuid::Uuid::from_bytes(dbid)),
        ))
    }
}

/// Key: (table_id: [u8; 16], flex_id: FlexId) のコーデック
pub struct TableIdAndFlexId;

impl<'a> heed::BytesEncode<'a> for TableIdAndFlexId {
    type EItem = (crate::models::id::TableId, FlexId);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(30);
        bytes.extend_from_slice(&item.0.into_bytes());
        bytes.extend_from_slice(&item.1.spatial_encode());
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for TableIdAndFlexId {
    type DItem = (crate::models::id::TableId, FlexId);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() != 30 {
            return Err("invalid length for TableIdAndFlexId".into());
        }
        let mut table_id = [0u8; 16];
        table_id.copy_from_slice(&bytes[0..16]);
        let table_id = crate::models::id::TableId(uuid::Uuid::from_bytes(table_id));

        let mut flex_id_bytes = [0u8; 14];
        flex_id_bytes.copy_from_slice(&bytes[16..30]);
        let flex_id = FlexId::spatial_decode(&flex_id_bytes)?;

        Ok((table_id, flex_id))
    }
}

/// アプリケーション全体のLMDB環境と各データベースのハンドルを保持する構造体
#[derive(Clone)]
pub struct AppDb {
    /// LMDBの環境本体
    pub env: Env<heed::WithoutTls>,

    /// データベースの一覧とメタデータを管理する
    /// Key: データベース名 (`Str`) -> Value: `DatabaseMetadata`
    pub databases: Database<Str, SerdeBincode<DatabaseMetadata>>,

    /// テーブルの一覧とメタデータを管理する
    /// Key: `DatabaseId` と テーブル名 (`DbIdAndName`) -> Value: `TableMetadata`
    pub tables: Database<DbIdAndName, SerdeBincode<TableMetadata>>,

    /// `TableId` の存在確認やルックアップに用いるインデックス
    /// Key: `TableId` -> Value: `Unit` (値なし)
    pub table_id_index: Database<SerdeBincode<crate::models::id::TableId>, Unit>,

    /// 登録済みのユーザーを管理する
    /// Key: ユーザーID（現状は UUID などの文字列）(`Str`) -> Value: ユーザー名など (`Str`)
    pub users: Database<Str, Str>,

    /// ユーザーごとのデータベースに対する権限（ロール）を管理する
    /// Key: `UserId` と `DatabaseId` (`UserIdAndDbId`) -> Value: 権限レベル (`UserRole` を表す `u8`)
    pub user_privileges: Database<UserIdAndDbId, SerdeBincode<u8>>,

    /// 各テーブルに紐づく実際の空間データ（GeoJSONフィーチャやタイル要素など）を格納する
    /// Key: `TableId` と 空間ID（`FlexId`）(`TableIdAndFlexId`) -> Value: BincodeやJSON等でシリアライズされた生データ (`Bytes`)
    pub tables_data: Database<TableIdAndFlexId, Bytes>,
}

#[tracing::instrument]
pub fn initialize_database(path: &str) -> AppDb {
    tracing::info!("Initializing database at: {}", path);
    std::fs::create_dir_all(path).unwrap();

    let map_size = std::env::var("KASANE_LMDB_MAP_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10 * 1024 * 1024 * 1024);

    // ランダムアクセス主体でワーキングセットが RAM を超える場合、OS の readahead が
    // ページキャッシュを汚して逆効果になることがある。その際は環境変数で無効化できる
    // （逐次レンジスキャン主体のワークロードでは付けない方が速いので既定は無効）。
    let no_readahead = std::env::var("KASANE_LMDB_NO_READAHEAD")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let env = unsafe {
        // TLS を無効化し、read トランザクションをスレッド間で共有可能にする
        // （1つの読みスナップショットを rayon の複数ワーカーから並列に読む）。
        let mut opts = EnvOpenOptions::new().read_txn_without_tls();
        opts.map_size(map_size).max_dbs(15);
        if no_readahead {
            opts.flags(EnvFlags::NO_READ_AHEAD);
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
    let table_id_index = env
        .create_database(&mut write_txn, Some("table_id_index"))
        .unwrap();
    let users = env.create_database(&mut write_txn, Some("users")).unwrap();
    let user_privileges = env
        .create_database(&mut write_txn, Some("user_privileges"))
        .unwrap();
    let tables_data = env
        .create_database(&mut write_txn, Some("tables_data"))
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
            is_global_admin: true,
            token_version: 0,
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
        table_id_index,
        users,
        user_privileges,
        tables_data,
    }
}
