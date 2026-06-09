use redb::{ReadableTable, WriteTransaction};

use crate::{
    db_init::{DATABASES, USER_PRIVILEGES, USERS},
    error::AppError,
    models::users::{UserMetadata, UserRole},
};

pub struct KasaneUsersWrite {
    write_txn: WriteTransaction,
}

impl KasaneUsersWrite {
    pub fn new(write_txn: WriteTransaction) -> Self {
        Self { write_txn }
    }

    pub fn create_user(&mut self, username: &str, meta: &UserMetadata) -> Result<(), AppError> {
        let mut users_table = self.write_txn.open_table(USERS)?;
        if users_table.get(username)?.is_some() {
            return Err(AppError::Conflict("User already exists".to_string()));
        }
        let json = serde_json::to_string(meta)
            .map_err(|_| AppError::InternalError("Failed to serialize user metadata".into()))?;
        users_table.insert(username, json.as_str())?;
        Ok(())
    }

    pub fn update_user_meta(
        &mut self,
        username: &str,
        meta: &UserMetadata,
    ) -> Result<(), AppError> {
        let mut users_table = self.write_txn.open_table(USERS)?;
        if users_table.get(username)?.is_none() {
            return Err(AppError::NotFound("User not found".to_string()));
        }
        let json = serde_json::to_string(meta)
            .map_err(|_| AppError::InternalError("Failed to serialize user metadata".into()))?;
        users_table.insert(username, json.as_str())?;
        Ok(())
    }

    pub fn delete_user(&mut self, username: &str) -> Result<(), AppError> {
        let mut users_table = self.write_txn.open_table(USERS)?;
        if let Some(val) = users_table.remove(username)? {
            let meta: UserMetadata = serde_json::from_str(val.value())
                .map_err(|_| AppError::InternalError("Failed to parse user metadata".into()))?;
            let user_id = meta.id.into_bytes();

            // cascade delete privileges
            let mut privs_table = self.write_txn.open_table(USER_PRIVILEGES)?;
            let keys: Vec<_> = privs_table
                .range((user_id, [0u8; 16])..=(user_id, [255u8; 16]))?
                .map(|res| res.map(|(k, _)| k.value()))
                .collect::<Result<Vec<_>, _>>()?;
            for k in keys {
                privs_table.remove(k)?;
            }
            Ok(())
        } else {
            Err(AppError::NotFound("User not found".to_string()))
        }
    }

    pub fn set_privilege(
        &mut self,
        user_id: [u8; 16],
        db_name: &str,
        role: UserRole,
    ) -> Result<(), AppError> {
        let dbs_table = self.write_txn.open_table(DATABASES)?;
        let db_id = if let Some(val) = dbs_table.get(db_name)? {
            val.value().id.into_bytes()
        } else {
            return Err(AppError::NotFound("Database not found".to_string()));
        };

        let mut privs_table = self.write_txn.open_table(USER_PRIVILEGES)?;
        privs_table.insert((user_id, db_id), role as u8)?;
        Ok(())
    }

    pub fn remove_privilege(&mut self, user_id: [u8; 16], db_name: &str) -> Result<(), AppError> {
        let dbs_table = self.write_txn.open_table(DATABASES)?;
        let db_id = if let Some(val) = dbs_table.get(db_name)? {
            val.value().id.into_bytes()
        } else {
            return Err(AppError::NotFound("Database not found".to_string()));
        };

        let mut privs_table = self.write_txn.open_table(USER_PRIVILEGES)?;
        privs_table.remove((user_id, db_id))?;
        Ok(())
    }

    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}
