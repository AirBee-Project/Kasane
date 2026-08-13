//! 書き込みトランザクション上で行える操作。

use kasane_logic::SpatialIdSet;

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, UpdateTableConstraints,
};
use crate::models::id::TableId;
use crate::models::users::{PrivilegeRule, PrivilegeTarget};

use super::CatalogRepository;

// `async fn` の Future に `Send` を課さない理由は [`Storage`](super::Storage) を参照。
#[allow(async_fn_in_trait)]
pub trait WriteRepository: CatalogRepository {
    // 作成前の重複確認のため「同じトランザクション内で読んでから書く」経路も要る。

    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError>;

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError>;

    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError>;

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError>;

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<(), AppError>;

    async fn database_copy(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError>;

    /// `value_index`: 値インデックスを維持するか。作成後は変更できない。
    #[allow(clippy::too_many_arguments)]
    async fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        value_index: bool,
    ) -> Result<Table, AppError>;

    #[allow(clippy::too_many_arguments)]
    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
        validate_existing_data: bool,
    ) -> Result<Table, AppError>;

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError>;

    async fn table_copy(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError>;

    /// `index` は値インデックスへ反映する型。`None` なら索引を維持しない。
    ///
    /// 「索引するか」と「どう索引するか」を 1 引数にまとめてあるのは、書き込み経路が型を
    /// 必要とするのが索引キーの順序保存エンコードだけだから。
    async fn data_insert(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    /// `entries` は `(空間 ID, 値)` の並び。同じ空間 ID が複数回現れたら後勝ち。
    ///
    /// 1 件ずつ [`data_insert`](Self::data_insert) を呼ぶのと結果は同じだが、シャードを
    /// 1 度しか読み書きしない。このツリーは 1 件の変更でもリーフを丸ごと書き直すので、
    /// 別々に書くと「リーフのサイズ × 件数」を書くことになる。
    async fn data_insert_many(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        entries: Vec<(SpatialIdSet, Vec<u8>)>,
    ) -> Result<(), AppError>;

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    async fn data_remove(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
    ) -> Result<(), AppError>;

    async fn create_user(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<(), AppError>;

    async fn set_password(&mut self, username: &str, password_hash: String)
    -> Result<(), AppError>;

    async fn grant_privilege(
        &mut self,
        username: &str,
        rule: &PrivilegeRule,
    ) -> Result<(), AppError>;

    async fn revoke_privilege(
        &mut self,
        username: &str,
        target: &PrivilegeTarget,
    ) -> Result<(), AppError>;

    async fn delete_user(&mut self, username: &str) -> Result<(), AppError>;
}
