use crate::{
    error::AppError,
    models::users::{User, UserMetadata, UserRole},
};
use heed::BytesDecode;

pub struct KasaneUsersRead<'a> {
    read_txn: heed::RoTxn<'a, heed::WithoutTls>,
    db: &'a crate::db_init::AppDb,
}

impl<'a> KasaneUsersRead<'a> {
    #[tracing::instrument(skip_all)]
    pub fn new(read_txn: heed::RoTxn<'a, heed::WithoutTls>, db: &'a crate::db_init::AppDb) -> Self {
        Self { read_txn, db }
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub fn get_user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError> {
        let users_table = self.db.users;
        if let Some(val) = users_table.get(&self.read_txn, username)? {
            let meta: UserMetadata = serde_json::from_str(val)
                .map_err(|_| AppError::InternalError("Failed to parse user metadata".into()))?;
            Ok(Some(meta))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub fn get_user(&self, username: &str) -> Result<Option<User>, AppError> {
        if let Some(meta) = self.get_user_meta(username)? {
            Ok(Some(User::from_meta(username, &meta)))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name))]
    pub fn get_privilege(
        &self,
        user_id: crate::models::id::UserId,
        db_name: &str,
    ) -> Result<Option<UserRole>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let dbs_table = self.db.databases;
        let db_id = if let Some(val) = dbs_table.get(&self.read_txn, db_name)? {
            val.id.into_bytes()
        } else {
            return Ok(None);
        };

        let privs_table = self.db.user_privileges;
        if let Some(val) = privs_table.get(
            &self.read_txn,
            &(
                user_id,
                crate::models::id::DatabaseId(uuid::Uuid::from_bytes(db_id)),
            ),
        )? {
            Ok(UserRole::from_u8(val))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn get_all_users(&self) -> Result<Vec<User>, AppError> {
        let users_table = self.db.users;
        let mut users = Vec::new();
        for item in users_table.iter(&self.read_txn)? {
            let (username, val) = item?;
            let meta: UserMetadata = serde_json::from_str(val)
                .map_err(|_| AppError::InternalError("Failed to parse user metadata".into()))?;
            users.push(User::from_meta(username, &meta));
        }
        Ok(users)
    }

    #[tracing::instrument(skip_all)]
    pub fn get_user_privileges(
        &self,
        user_id: crate::models::id::UserId,
    ) -> Result<Vec<(String, UserRole)>, AppError> {
        let dbs_table = self.db.databases;
        let mut db_id_to_name = std::collections::HashMap::new();
        for item in dbs_table.iter(&self.read_txn)? {
            let (k, v) = item?;
            db_id_to_name.insert(v.id, k.to_string());
        }

        let privs_table = self.db.user_privileges;
        let mut res = Vec::new();
        for item in privs_table
            .remap_key_type::<heed::types::Bytes>()
            .prefix_iter(&self.read_txn, &user_id.into_bytes())?
        {
            let (k_bytes, val) = item?;
            let (_, db_id) = crate::db_init::UserIdAndDbId::bytes_decode(k_bytes).unwrap();
            if let Some(db_name) = db_id_to_name.get(&db_id)
                && let Some(role) = UserRole::from_u8(val)
            {
                res.push((db_name.clone(), role));
            }
        }
        Ok(res)
    }
}
