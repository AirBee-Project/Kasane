//! 読み取りトランザクション上で行える操作。

use kasane_logic::{FlexId, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{Table, TableDataType};
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::User;

use super::{CatalogRepository, ValueGroups};

/// 名前から解決した `(データベース, テーブル)`。
///
/// 「無い」ことをエラーにしないのは、認可より先に存在有無を返すと権限の無い利用者へ
/// 名前の存在を教えてしまうため。
#[derive(Debug, Clone)]
pub struct ResolvedTable {
    pub db_id: Option<DatabaseId>,
    pub table: Option<Table>,
}

// `async fn` の Future に `Send` を課さない理由は [`Storage`](super::Storage) を参照。
#[allow(async_fn_in_trait)]
pub trait ReadRepository: CatalogRepository {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError>;

    /// 呼び出し側が権限の絞り込みに使うので、ID を添えて返す。
    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError>;

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError>;

    /// `(データベース名, テーブル名)` をまとめて解決する。返りは入力と同じ並び。
    ///
    /// 名前の解決はバックエンドによっては 1 件ずつがネットワーク往復なので、まとめて
    /// 渡せる形にしてある。
    async fn resolve_tables(
        &self,
        refs: &[(String, String)],
    ) -> Result<Vec<ResolvedTable>, AppError>;

    async fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError>;

    /// ID を解決済みの呼び出し側が、名前からの引き直しを避けるために使う。
    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError>;

    async fn table_count(&self, table_id: TableId) -> Result<u64, AppError>;

    async fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError>;

    async fn data_filter_eq(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError>;

    /// `lo`〜`hi` は両端を含む。
    async fn data_filter_range(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError>;

    async fn get_all_users(&self) -> Result<Vec<User>, AppError>;

    async fn get_user(&self, username: &str) -> Result<Option<User>, AppError> {
        Ok(self
            .user_meta(username)
            .await?
            .map(|meta| User::from_meta(username, meta)))
    }

    async fn require_user(&self, username: &str) -> Result<User, AppError> {
        Ok(User::from_meta(
            username,
            self.require_user_meta(username).await?,
        ))
    }
}
