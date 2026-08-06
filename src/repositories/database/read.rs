use crate::{error::AppError, models::database::DatabaseInfoResponse, repositories::KasaneDbRead};

impl<'a> KasaneDbRead<'a> {
    /// Databaseの情報を取得する
    #[tracing::instrument(skip_all)]
    pub fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        if name.is_empty() {
            return Ok(None);
        }
        let db = self.db.databases;
        if let Some(meta) = db.get(&self.read_txn, name)? {
            Ok(Some(DatabaseInfoResponse {
                name: name.to_string(),
                description: meta.description,
            }))
        } else {
            Ok(None)
        }
    }

    /// Databaseの一覧を取得する
    #[tracing::instrument(skip_all)]
    pub fn database_list(&self) -> Result<Vec<DatabaseInfoResponse>, AppError> {
        let db = self.db.databases;
        let mut list = Vec::new();
        for res in db.iter(&self.read_txn)? {
            let (k, meta) = res.map_err(AppError::from)?;
            list.push(DatabaseInfoResponse {
                name: k.to_string(),
                description: meta.description,
            });
        }
        Ok(list)
    }
}
