pub mod database;
pub mod users;

use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};

use std::collections::HashMap;

pub struct KasaneDbRead<'a> {
    pub read_txn: heed::RoTxn<'a, heed::WithoutTls>,
    pub db: &'a crate::db_init::AppDb,
}

impl<'a> KasaneDbRead<'a> {
    pub fn new(read_txn: heed::RoTxn<'a, heed::WithoutTls>, db: &'a crate::db_init::AppDb) -> Self {
        Self { read_txn, db }
    }
}

pub struct KasaneDbWrite<'a> {
    pub write_txn: heed::RwTxn<'a>,
    pub db: &'a crate::db_init::AppDb,
    pub database_caches: HashMap<String, DatabaseMetadata>,
    pub table_caches: HashMap<(crate::models::id::DatabaseId, String), TableMetadata>,
}

impl<'a> KasaneDbWrite<'a> {
    pub fn new(write_txn: heed::RwTxn<'a>, db: &'a crate::db_init::AppDb) -> Self {
        Self {
            write_txn,
            db,
            database_caches: HashMap::new(),
            table_caches: std::collections::HashMap::new(),
        }
    }

    pub fn commit(self) -> Result<(), crate::error::AppError> {
        self.write_txn.commit()?;
        Ok(())
    }

    pub fn abort(self) -> Result<(), crate::error::AppError> {
        self.write_txn.abort();
        Ok(())
    }
}

impl crate::db_init::AppDb {
    pub async fn batch_data_insert(
        &self,
        db_name: String,
        table_name: String,
        ids: Vec<crate::models::spatial_id::SpatialId>,
        policy: crate::models::database::table::data::ZoomLevelPolicy,
        value: serde_json::Value,
    ) -> Result<(), crate::error::AppError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.write_sender
            .send(
                crate::repositories::database::table::data::batch::WriteOp::Insert {
                    db_name,
                    table_name,
                    ids,
                    policy,
                    value,
                    reply: tx,
                },
            )
            .await
            .map_err(|_| {
                crate::error::AppError::InternalError("WriteBatcher channel is closed".to_string())
            })?;
        rx.await.map_err(|_| {
            crate::error::AppError::InternalError("WriteBatcher failed to reply".to_string())
        })??;
        Ok(())
    }

    pub async fn batch_data_upsert(
        &self,
        db_name: String,
        table_name: String,
        ids: Vec<crate::models::spatial_id::SpatialId>,
        policy: crate::models::database::table::data::ZoomLevelPolicy,
        value: serde_json::Value,
    ) -> Result<(), crate::error::AppError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.write_sender
            .send(
                crate::repositories::database::table::data::batch::WriteOp::Upsert {
                    db_name,
                    table_name,
                    ids,
                    policy,
                    value,
                    reply: tx,
                },
            )
            .await
            .map_err(|_| {
                crate::error::AppError::InternalError("WriteBatcher channel is closed".to_string())
            })?;
        rx.await.map_err(|_| {
            crate::error::AppError::InternalError("WriteBatcher failed to reply".to_string())
        })??;
        Ok(())
    }

    pub async fn batch_data_remove(
        &self,
        db_name: String,
        table_name: String,
        ids: Vec<crate::models::spatial_id::SpatialId>,
        policy: crate::models::database::table::data::ZoomLevelPolicy,
    ) -> Result<(), crate::error::AppError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.write_sender
            .send(
                crate::repositories::database::table::data::batch::WriteOp::Remove {
                    db_name,
                    table_name,
                    ids,
                    policy,
                    reply: tx,
                },
            )
            .await
            .map_err(|_| {
                crate::error::AppError::InternalError("WriteBatcher channel is closed".to_string())
            })?;
        rx.await.map_err(|_| {
            crate::error::AppError::InternalError("WriteBatcher failed to reply".to_string())
        })??;
        Ok(())
    }
}
