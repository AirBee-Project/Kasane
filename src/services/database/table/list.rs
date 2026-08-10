use crate::repositories::{CatalogRepository, ReadRepository, Storage};
use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableListResponse, TableSummary},
    models::users::{Scope, User, UserRole},
};

/// テーブル一覧を返す。呼び出したユーザーが Read 以上で到達できるテーブルだけに絞る。
///
/// データベースレベル以上の権限を持つユーザーには全件が見え、テーブル単位の権限しか
/// 持たないユーザーには該当テーブルだけが見える。
#[tracing::instrument(skip_all, fields(db_name = %db_name))]
pub async fn list(
    app_state: &AppState,
    db_name: &str,
    user: &User,
) -> Result<TableListResponse, AppError> {
    let owned = db_name.to_string();
    let (db_id, tables) = app_state
        .db
        .read(async move |db| {
            let db_id = db.require_database_id(&owned).await?;
            Ok((db_id, db.table_list_by_id(db_id).await?))
        })
        .await?;

    let response_tables = tables
        .into_iter()
        .filter(|table| user.can(Scope::Table(db_id, table.id), UserRole::Read))
        .map(|table| TableSummary {
            name: table.name,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            constraints: table.constraints,
            description: table.description,
        })
        .collect();
    Ok(TableListResponse(response_tables))
}
