use crate::UserError;
use sled::{Db, Tree};
use std::{env, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
pub mod create_key;
pub mod create_space;
pub mod info_key;
pub mod info_space;
pub mod insert_value;
pub mod show_keys;
pub mod show_spaces;
pub mod tools;
pub mod verify_user;
pub mod version;
pub struct Storage {
    pub db: Db,
    pub space: Tree,
    pub key: Tree,
}

impl Storage {
    pub fn new(path: Option<PathBuf>) -> Result<Self, UserError> {
        let db_path = path.unwrap_or(env::current_dir().unwrap().join("sled_db"));

        let config = sled::Config::default()
            .path(db_path)
            .cache_capacity(10_000_000_000)
            .flush_every_ms(Some(1000))
            .mode(sled::Mode::HighThroughput);

        let db = config.open()?;
        let space = db.open_tree("space")?;
        let key = db.open_tree("key")?;

        Ok(Self { db, key, space })
    }
}
