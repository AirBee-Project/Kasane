use crate::{
    AppState,
    error::AppError,
    models::database::table::Table,
    models::users::{User, UserRole},
    repositories::{Storage, WriteRepository},
    services::auth::authorize_path,
};

/// 複製元には Read、複製**先のデータベース**には Manage を要求する。
///
/// 作られるのは新しいテーブルなので、複製元へのテーブルスコープでは通さない。
#[tracing::instrument(skip_all, fields(src_db_name = %src_db_name, src_table_name = %src_table_name, copy_db_name = %copy_db_name, copy_table_name = %copy_table_name))]
pub async fn copy(
    app_state: &AppState,
    user: &User,
    src_db_name: &str,
    src_table_name: &str,
    copy_db_name: &str,
    copy_table_name: &str,
) -> Result<Table, AppError> {
    let src_db_name = src_db_name.to_string();
    let src_table_name = src_table_name.to_string();
    let copy_db_name = copy_db_name.to_string();
    let copy_table_name = copy_table_name.to_string();
    let user = user.clone();

    app_state
        .db
        .write(async move |db| {
            authorize_path(
                db,
                &user,
                &src_db_name,
                Some(&src_table_name),
                UserRole::Read,
            )
            .await?;
            authorize_path(db, &user, &copy_db_name, None, UserRole::Manage).await?;

            db.table_copy(
                &src_db_name,
                &src_table_name,
                &copy_db_name,
                &copy_table_name,
            )
            .await
        })
        .await
}
