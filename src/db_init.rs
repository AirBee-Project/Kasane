use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};

use crate::models::users::UserMetadata;
use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};
use crate::services::auth::hash_password;
use std::borrow::Cow;
use uuid::Uuid;

pub struct DbIdAndName;

impl<'a> heed::BytesEncode<'a> for DbIdAndName {
    type EItem = ([u8; 16], &'a str);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(16 + item.1.len());
        bytes.extend_from_slice(&item.0);
        bytes.extend_from_slice(item.1.as_bytes());
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for DbIdAndName {
    type DItem = ([u8; 16], &'a str);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() < 16 {
            return Err("invalid length".into());
        }
        let mut id = [0u8; 16];
        id.copy_from_slice(&bytes[0..16]);
        let name = std::str::from_utf8(&bytes[16..])?;
        Ok((id, name))
    }
}

pub struct TableIdAndSpatialId;

impl<'a> heed::BytesEncode<'a> for TableIdAndSpatialId {
    type EItem = ([u8; 16], [u8; 12]);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(28);
        bytes.extend_from_slice(&item.0);
        bytes.extend_from_slice(&item.1);
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for TableIdAndSpatialId {
    type DItem = ([u8; 16], [u8; 12]);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() != 28 {
            return Err("invalid length".into());
        }
        let mut id = [0u8; 16];
        id.copy_from_slice(&bytes[0..16]);
        let mut spatial = [0u8; 12];
        spatial.copy_from_slice(&bytes[16..28]);
        Ok((id, spatial))
    }
}

pub struct ValueToSpatialId;

impl<'a> heed::BytesEncode<'a> for ValueToSpatialId {
    type EItem = ([u8; 16], &'a [u8], [u8; 12]);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(16 + item.1.len() + 12);
        bytes.extend_from_slice(&item.0);
        bytes.extend_from_slice(item.1);
        bytes.extend_from_slice(&item.2);
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for ValueToSpatialId {
    type DItem = ([u8; 16], &'a [u8], [u8; 12]);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() < 28 {
            return Err("invalid length".into());
        }
        let mut id = [0u8; 16];
        id.copy_from_slice(&bytes[0..16]);
        let value = &bytes[16..bytes.len() - 12];
        let mut spatial = [0u8; 12];
        spatial.copy_from_slice(&bytes[bytes.len() - 12..]);
        Ok((id, value, spatial))
    }
}

pub struct UserIdAndDbId;

impl<'a> heed::BytesEncode<'a> for UserIdAndDbId {
    type EItem = ([u8; 16], [u8; 16]);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&item.0);
        bytes.extend_from_slice(&item.1);
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for UserIdAndDbId {
    type DItem = ([u8; 16], [u8; 16]);

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
        Ok((uid, dbid))
    }
}

#[derive(Clone)]
pub struct AppDb {
    pub env: Env,
    pub databases: Database<Str, SerdeBincode<DatabaseMetadata>>,
    pub tables: Database<DbIdAndName, SerdeBincode<TableMetadata>>,
    pub table_id_index: Database<SerdeBincode<[u8; 16]>, Unit>,
    pub spatialid_to_value: Database<TableIdAndSpatialId, Bytes>,
    pub value_to_spatialid: Database<ValueToSpatialId, Unit>,
    pub users: Database<Str, Str>,
    pub user_privileges: Database<UserIdAndDbId, SerdeBincode<u8>>,
}

#[tracing::instrument]
pub fn initialize_database(path: &str) -> AppDb {
    tracing::info!("Initializing database at: {}", path);
    std::fs::create_dir_all(path).unwrap();

    let map_size = std::env::var("KASANE_LMDB_MAP_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10 * 1024 * 1024 * 1024);

    let env = unsafe {
        EnvOpenOptions::new()
            .map_size(map_size)
            .max_dbs(10)
            .open(path)
            .unwrap_or_else(|e| {
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
    let spatialid_to_value = env
        .create_database(&mut write_txn, Some("spatialid_to_value"))
        .unwrap();
    let value_to_spatialid = env
        .create_database(&mut write_txn, Some("value_to_spatialid"))
        .unwrap();
    let users = env.create_database(&mut write_txn, Some("users")).unwrap();
    let user_privileges = env
        .create_database(&mut write_txn, Some("user_privileges"))
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
        spatialid_to_value,
        value_to_spatialid,
        users,
        user_privileges,
    }
}
