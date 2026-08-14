//! 読み取りトランザクション上で行える操作。

use kasane_logic::{FlexId, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{Table, TableDataType};
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::{User, UserRecord};

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
    /// **ID も返す。** 呼び出し側が認可のために同じキーをもう一度引かずに済む。
    async fn database_info(
        &self,
        name: &str,
    ) -> Result<Option<(DatabaseId, DatabaseInfoResponse)>, AppError>;

    /// 全データベース。**全体権限を持つ利用者にしか使わない。**
    ///
    /// 権限で絞る側は [`databases_by_id`](Self::databases_by_id) を使う。全件取ってから
    /// 捨てる形だと、権限を持たない対象のコストまで払うことになる。
    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError>;

    /// ID を指定して引く。ACL 側から辿った一覧の組み立てに使う。
    ///
    /// 存在しない ID は結果に現れない。
    async fn databases_by_id(
        &self,
        ids: &[DatabaseId],
    ) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError>;

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

    /// このデータベース配下の、指定した ID のテーブルだけ。
    ///
    /// テーブル単位の権限しか持たない利用者の一覧に使う。配下全件を舐めない。
    async fn tables_by_id(
        &self,
        db_id: DatabaseId,
        ids: &[TableId],
    ) -> Result<Vec<Table>, AppError>;

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

    /// 利用者名の辞書順で 1 ページぶん返す。
    ///
    /// `after` より後ろから最大 `limit` 件。全件を持ち回らないのは、1 リクエストの
    /// 読み取りを利用者数に比例させないため。
    async fn list_users(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, UserRecord)>, AppError>;

    async fn get_user(&self, username: &str) -> Result<Option<User>, AppError> {
        Ok(self
            .user_record(username)
            .await?
            .map(|record| User::from_record(username, record)))
    }

    async fn require_user(&self, username: &str) -> Result<User, AppError> {
        Ok(User::from_record(
            username,
            self.require_user_record(username).await?,
        ))
    }
}
