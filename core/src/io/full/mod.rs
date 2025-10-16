use crate::UserError;
use sled::Db;
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
    pub lock: Arc<Mutex<()>>,
}

impl Storage {
    pub fn new(path: Option<PathBuf>) -> Result<Self, UserError> {
        let db_path = path.unwrap_or(env::current_dir().unwrap().join("sled_db"));
        let db = sled::open(&db_path)?;
        Ok(Self {
            db,
            lock: Arc::new(Mutex::new(())),
        })
    }
}
