use crate::UserError;
use sled::{
    Db, Tree,
    transaction::{
        ConflictableTransactionResult, TransactionError, TransactionalTree, TransactionalTrees,
    },
};
use std::{env, path::PathBuf};
pub mod create_key;
pub mod create_space;
pub mod create_user;
pub mod grant_database;
pub mod tools;

use sled::Transactional;
pub struct Storage {
    pub db: Db,
}

impl Storage {
    pub fn new(path: Option<PathBuf>) -> Result<Self, UserError> {
        // sledデータベースを開く（ディレクトリがなければ作成）
        let db_path = path.unwrap_or(env::current_dir().unwrap().join("sled_db"));
        let db = sled::open(&db_path)?;
        Ok(Self { db })
    }
}
