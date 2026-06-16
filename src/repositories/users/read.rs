use crate::{
    db_init::{DATABASES, USER_PRIVILEGES, USERS},
    error::AppError,
    models::users::{User, UserMetadata, UserRole},
};
use redb::{ReadTransaction, ReadableTable};

pub struct KasaneUsersRead {
    read_txn: ReadTransaction,
}

impl KasaneUsersRead {
    pub fn new(read_txn: ReadTransaction) -> Self {
        Self { read_txn }
    }

    pub fn get_user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError> {
        let users_table = self.read_txn.open_table(USERS)?;
        if let Some(val) = users_table.get(username)? {
            let meta: UserMetadata = serde_json::from_str(val.value())
                .map_err(|_| AppError::InternalError("Failed to parse user metadata".into()))?;
            Ok(Some(meta))
        } else {
            Ok(None)
        }
    }

    pub fn get_user(&self, username: &str) -> Result<Option<User>, AppError> {
        if let Some(meta) = self.get_user_meta(username)? {
            Ok(Some(User::from_meta(username, &meta)))
        } else {
            Ok(None)
        }
    }

    pub fn get_privilege(
        &self,
        user_id: [u8; 16],
        db_name: &str,
    ) -> Result<Option<UserRole>, AppError> {
        let dbs_table = self.read_txn.open_table(DATABASES)?;
        let db_id = if let Some(val) = dbs_table.get(db_name)? {
            val.value().id.into_bytes()
        } else {
            return Ok(None);
        };

        let privs_table = self.read_txn.open_table(USER_PRIVILEGES)?;
        if let Some(val) = privs_table.get((user_id, db_id))? {
            Ok(UserRole::from_u8(val.value()))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_users(&self) -> Result<Vec<User>, AppError> {
        let users_table = self.read_txn.open_table(USERS)?;
        let mut users = Vec::new();
        for item in users_table.iter()? {
            let (key, val) = item?;
            let username = key.value();
            let meta: UserMetadata = serde_json::from_str(val.value())
                .map_err(|_| AppError::InternalError("Failed to parse user metadata".into()))?;
            users.push(User::from_meta(username, &meta));
        }
        Ok(users)
    }

    pub fn get_user_privileges(
        &self,
        user_id: [u8; 16],
    ) -> Result<Vec<(String, UserRole)>, AppError> {
        let dbs_table = self.read_txn.open_table(DATABASES)?;
        let mut db_id_to_name = std::collections::HashMap::new();
        for item in dbs_table.iter()? {
            let (k, v) = item?;
            db_id_to_name.insert(v.value().id.into_bytes(), k.value().to_string());
        }

        let privs_table = self.read_txn.open_table(USER_PRIVILEGES)?;
        let mut res = Vec::new();
        for item in privs_table.range((user_id, [0u8; 16])..=(user_id, [255u8; 16]))? {
            let (k, v) = item?;
            let db_id = k.value().1;
            if let Some(db_name) = db_id_to_name.get(&db_id)
                && let Some(role) = UserRole::from_u8(v.value())
            {
                res.push((db_name.clone(), role));
            }
        }
        Ok(res)
    }
}
