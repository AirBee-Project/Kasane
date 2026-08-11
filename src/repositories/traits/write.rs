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

/// 書き込みトランザクション上で行える操作。
///
/// 読み取り系の一部（`database_info` / `table_info`）も併せ持つのは、作成前の重複確認など
/// 「同じトランザクション内で読んでから書く」処理が必要なため。
// `async fn` の戻り値の Future には呼び出し側から `Send` 境界を付けられない。
// このアプリではバックエンドが feature で 1 つに確定し、Send 性は具体型経由で
// 漏れ出すため、trait 側で境界を要求する必要がない（`storage.rs` の設計メモを参照）。
// 署名の読みやすさを優先して `async fn` を使う。
#[allow(async_fn_in_trait)]
pub trait WriteRepository: CatalogRepository {
    // --- 同一トランザクション内での確認用の読み取り ---

    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError>;

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError>;

    // --- データベース ---

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

    // --- テーブル ---

    #[allow(clippy::too_many_arguments)]
    async fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        // `value_index`: 値インデックスを維持するか。作成後は変更できない。
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

    // --- データ ---

    /// `index` は値インデックスへ反映する型。`None` なら索引を維持しない。
    ///
    /// 書き込み経路が型を必要とするのは索引キーの順序保存エンコードのためだけなので、
    /// 「索引するか」と「どう索引するか」を 1 つの引数にまとめてある
    /// （[`Table::value_indexing`](crate::models::database::table::Table::value_indexing)）。
    async fn data_insert(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
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

    // --- ユーザーと権限 ---

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
