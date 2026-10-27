//! 書き込みトランザクション上で行える操作。

use spatial_id::collection::SpatialIdSet;

use crate::error::AppError;
use crate::models::database::DatabaseDetail;
use crate::models::id::TableId;
use crate::models::table::{Table, TableConstraints, TableDataType};

use super::CatalogRepository;

// `async fn` の Future に `Send` を課さない理由は [`Storage`](super::Storage) を参照。
#[allow(async_fn_in_trait)]
pub trait WriteRepository: CatalogRepository {
    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseDetail, AppError>;

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError>;

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<DatabaseDetail, AppError>;

    #[allow(clippy::too_many_arguments)]
    async fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        is_temporal: bool,
    ) -> Result<Table, AppError>;

    /// `is_temporal`: `Some(true)` で時間ロックを解除する。`Some(false)`（再ロック）は
    /// 既存データに時間成分が残っていないかを定数時間で確認する手段が無いため未対応で、サービスレイヤーが事前に弾く。
    #[allow(clippy::too_many_arguments)]
    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<TableConstraints>>,
        description: Option<Option<String>>,
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError>;

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError>;

    async fn data_insert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    async fn data_remove(&mut self, table_id: TableId, ids: SpatialIdSet) -> Result<(), AppError>;
}
