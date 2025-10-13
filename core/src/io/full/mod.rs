pub mod create_key;
pub mod create_space;
pub mod create_user;
pub mod grant_database;
pub mod tools;

use std::{env, path::PathBuf};

use crate::io::StorageTrait;
use lmdb::{Cursor, DatabaseFlags};

use super::UserError;
use lmdb::{Database, Environment, Transaction};

pub struct Storage {
    pub space: Database,
    pub key: Database,
    pub value: Database,
    pub user: Database,
    pub grant: Database,
    pub env: Environment,
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
