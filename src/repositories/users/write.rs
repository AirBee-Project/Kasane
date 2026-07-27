use crate::{
    error::AppError,
    models::users::{UserMetadata, UserRole},
};
use heed::BytesDecode;

pub struct KasaneUsersWrite<'a> {
    write_txn: heed::RwTxn<'a>,
    db: &'a crate::db_init::AppDb,
}

impl<'a> KasaneUsersWrite<'a> {
    #[tracing::instrument(skip_all)]
    pub fn new(write_txn: heed::RwTxn<'a>, db: &'a crate::db_init::AppDb) -> Self {
        Self { write_txn, db }
    }

    #[tracing::instrument(skip_all)]
    pub fn create_user(&mut self, username: &str, meta: &UserMetadata) -> Result<(), AppError> {
        let users_table = self.db.users;
        if users_table.get(&self.write_txn, username)?.is_some() {
            return Err(AppError::Conflict("User already exists".to_string()));
        }
        let json = serde_json::to_string(meta)
            .map_err(|_| AppError::InternalError("Failed to serialize user metadata".into()))?;
        users_table.put(&mut self.write_txn, username, json.as_str())?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn update_user_meta(
        &mut self,
        username: &str,
        meta: &UserMetadata,
    ) -> Result<(), AppError> {
        let users_table = self.db.users;
        if users_table.get(&self.write_txn, username)?.is_none() {
            return Err(AppError::NotFound("User not found".to_string()));
        }
        let json = serde_json::to_string(meta)
            .map_err(|_| AppError::InternalError("Failed to serialize user metadata".into()))?;
        users_table.put(&mut self.write_txn, username, json.as_str())?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn delete_user(&mut self, username: &str) -> Result<(), AppError> {
        let users_table = self.db.users;
        if let Some(val) = users_table.get(&self.write_txn, username)? {
            let meta: UserMetadata = serde_json::from_str(val)
                .map_err(|_| AppError::InternalError("Failed to parse user metadata".into()))?;
            let user_id = meta.id.into_bytes();

            users_table.delete(&mut self.write_txn, username)?;

            let privs_table = self.db.user_privileges;
            let mut keys = Vec::new();
            for item in privs_table
                .remap_key_type::<heed::types::Bytes>()
                .prefix_iter(&self.write_txn, user_id.as_slice())?
            {
                let (k, _) = item?;
                keys.push(crate::db_init::UserIdAndDbId::bytes_decode(k).unwrap());
            }
            for k in keys {
                privs_table.delete(&mut self.write_txn, &k)?;
            }
            Ok(())
        } else {
            Err(AppError::NotFound("User not found".to_string()))
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn set_privilege(
        &mut self,
        user_id: crate::models::id::UserId,
        db_name: &str,
        role: UserRole,
    ) -> Result<(), AppError> {
        if db_name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: db_name.to_string(),
            });
        }
        let dbs_table = self.db.databases;
        let db_id = if let Some(val) = dbs_table.get(&self.write_txn, db_name)? {
            val.id.into_bytes()
        } else {
            return Err(AppError::NotFound("Database not found".to_string()));
        };

        let privs_table = self.db.user_privileges;
        privs_table.put(
            &mut self.write_txn,
            &(
                user_id,
                crate::models::id::DatabaseId(uuid::Uuid::from_bytes(db_id)),
            ),
            &(role as u8),
        )?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn remove_privilege(
        &mut self,
        user_id: crate::models::id::UserId,
        db_name: &str,
    ) -> Result<(), AppError> {
        if db_name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: db_name.to_string(),
            });
        }
        let dbs_table = self.db.databases;
        let db_id = if let Some(val) = dbs_table.get(&self.write_txn, db_name)? {
            val.id.into_bytes()
        } else {
            return Err(AppError::NotFound("Database not found".to_string()));
        };

        let privs_table = self.db.user_privileges;
        privs_table.delete(
            &mut self.write_txn,
            &(
                user_id,
                crate::models::id::DatabaseId(uuid::Uuid::from_bytes(db_id)),
            ),
        )?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}
