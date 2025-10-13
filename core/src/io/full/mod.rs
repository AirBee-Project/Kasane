pub mod create_key;
pub mod create_space;

use std::{
    collections::{HashMap, HashSet},
    env,
    path::PathBuf,
};

use crate::{
    io::{
        StorageTrait, ValueEntry,
        tools::keytype_id::{id_keytype, keytype_id},
    },
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, InfoSpace, InfoUser, Output, ShowUsers, Showkeys},
    },
};
use argon2::password_hash::PasswordHasher;
use argon2::{Argon2, PasswordHash, PasswordVerifier, password_hash::SaltString};
use lmdb::{Cursor, DatabaseFlags, Error as LmdbError, WriteFlags};
use rand::rngs::OsRng;

use super::UserError;
use lmdb::{Database, Environment, Transaction};
use uuid::Uuid;

pub struct Storage {
    pub space: Database,
    pub key: Database,
    pub value: Database,
    pub user: Database,
    pub env: Environment,
}

impl From<lmdb::Error> for UserError {
    fn from(e: lmdb::Error) -> Self {
        match e {
            lmdb::Error::MapFull => UserError::LmdbMapFull {
                attempted_size: 0, // 必要に応じて Environment から取得して渡す
                location: "unknown",
            },
            lmdb::Error::NotFound => UserError::LmdbDbNotFound {
                db_name: "unknown",
                location: "unknown",
            },
            _ => UserError::LmdbError {
                message: e.to_string(),
                location: "unknown",
            },
        }
    }
}

impl From<std::str::Utf8Error> for UserError {
    fn from(e: std::str::Utf8Error) -> Self {
        UserError::ParseError {
            message: format!("Invalid UTF-8: {}", e),
            location: "unknown",
        }
    }
}

impl From<std::string::FromUtf8Error> for UserError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        UserError::ParseError {
            message: format!("Invalid UTF-8 (from Vec<u8>): {}", e),
            location: "unknown",
        }
    }
}
use std::convert::TryFrom;

impl TryFrom<u8> for KeyMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(KeyMode::UniqueKey),
            1 => Ok(KeyMode::MultiKey),
            _ => Err(()),
        }
    }
}

impl Storage {
    pub fn new(path: Option<PathBuf>) -> Result<Self, UserError> {
        // LMDB 環境を作成
        let env = Environment::new()
            .set_max_dbs(10) // 名前付きDBの上限
            .set_map_size(1024 * 1024 * 1024) // 1GB
            .open(&path.unwrap_or(env::current_dir().unwrap()))?;

        // データベースを開く（なければ作成）
        let space = env.create_db(Some("space"), DatabaseFlags::empty())?;
        let key = env.create_db(Some("key"), DatabaseFlags::empty())?;
        let value = env.create_db(Some("value"), DatabaseFlags::empty())?;
        let user = env.create_db(Some("user"), DatabaseFlags::empty())?;

        let storage = Self {
            space,
            key,
            value,
            user,
            env,
        };

        // === 初回起動時の admin ユーザー作成 ===
        {
            let txn = storage.env.begin_ro_txn()?;
            let admin_exists = txn.get(storage.user, b"admin").is_ok();
            drop(txn);

            if !admin_exists {
                // デフォルトパスワードは "admin" にしておく
                // 必要なら env から読み込むことも可能
                storage.create_user("admin", "nekocute")?;
                println!(
                    "✔ 初回起動: admin ユーザーを作成しました (username=admin, password=admin)"
                );
            }
        }

        Ok(storage)
    }
}
