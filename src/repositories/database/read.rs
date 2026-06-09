use redb::ReadableTable;

use crate::{
    db_init::DATABASES, error::AppError, models::database::DatabaseInfoResponse,
    repositories::KasaneDbRead,
};

impl KasaneDbRead {
    /// Databaseの情報を取得する
    pub fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        let redb_dbs = self.read_txn.open_table(DATABASES)?;
        if redb_dbs.get(name)?.is_some() {
            Ok(Some(DatabaseInfoResponse {
                name: name.to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Databaseの一覧を取得する
    pub fn database_list(&self) -> Result<Vec<DatabaseInfoResponse>, AppError> {
        self.read_txn
            .open_table(DATABASES)?
            .iter()?
            .map(|res| {
                let (k, _) = res.map_err(AppError::from)?;
                Ok(DatabaseInfoResponse {
                    name: k.value().to_owned(),
                })
            })
            .collect()
    }
}
