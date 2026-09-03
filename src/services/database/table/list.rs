use crate::services::auth::{reaches, visible_database};
use crate::models::users::{Scope, User, UserRole};
use crate::repositories::{CatalogRepository, ReadRepository, Storage};
use crate::{
    AppState,
    error::AppError,
    models::database::table::{TableListResponse, TableSummary},
};

/// テーブル一覧を返す。呼び出したユーザーが Read 以上で到達できるテーブルだけに絞る。
///
/// データベースレベル以上の権限を持つユーザーには全件が見え、テーブル単位の権限しか
/// 持たないユーザーには該当テーブルだけが見える。
///
/// **後者は権限の側から引く。** 配下を全件列挙してから捨てる形だと、1 テーブルしか
/// 見えない利用者もデータベース内のテーブル数に比例した読み取りを払うことになる。
#[tracing::instrument(skip_all, fields(db_name = %db_name))]
pub async fn list(
    app_state: &AppState,
    db_name: &str,
    user: &User,
) -> Result<TableListResponse, AppError> {
    let owned = db_name.to_string();
    let user = user.clone();

    let tables = app_state
        .db
        .read(async move |db| {
            let found = db.database_id(&owned).await?.map(|db_id| (db_id, ()));
            let (db_id, ()) = visible_database(db, &user, &owned, found).await?;

            // データベース全体に Read 以上で届くなら全件が対象。
            if reaches(db, &user, Scope::Database(db_id), UserRole::Read).await? {
                return db.table_list_by_id(db_id).await;
            }

            // ここへ来るのはテーブル単位の行しか持たない利用者だけ。
            let ids: Vec<_> = db
                .acl_tables_in(user.id, db_id)
                .await?
                .into_iter()
                .collect();
            db.tables_by_id(db_id, &ids).await
        })
        .await?;

    Ok(TableListResponse(
        tables
            .into_iter()
            .map(|table| TableSummary {
                name: table.name,
                data_type: table.data_type,
                max_zoom_level: table.max_zoom_level,
                constraints: table.constraints,
                description: table.description,
                is_temporal: table.is_temporal,
            })
            .collect(),
    ))
}
