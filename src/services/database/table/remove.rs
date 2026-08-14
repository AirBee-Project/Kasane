use crate::{
    AppState,
    error::AppError,
    middleware::auth::authorize_path,
    models::users::{User, UserRole},
    repositories::{Storage, WriteRepository},
};

/// 認可を書き込みトランザクションの中で行う。
///
/// 判定と削除が同じ断面なので、判定を通したテーブルが「同名で作り直された別のテーブル」
/// に化けたまま消される、という取り違えが起きない。
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn remove(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    table_name: &str,
) -> Result<(), AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let user = user.clone();

    app_state
        .db
        .write(async move |db| {
            authorize_path(db, &user, &db_name, Some(&table_name), UserRole::Manage).await?;
            db.table_remove(&db_name, &table_name).await
        })
        .await
}
