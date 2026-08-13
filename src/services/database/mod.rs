use crate::{
    AppState,
    error::AppError,
    models::database::DatabaseInfoResponse,
    models::users::{Scope, User, UserRole},
    repositories::{ReadRepository, Storage, WriteRepository},
};

#[tracing::instrument(skip_all, fields(db_name = %name))]
pub async fn info(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    let owned = name.to_string();
    match app_state
        .db
        .read(async move |r| r.database_info(&owned).await)
        .await?
    {
        Some(info) => Ok(info),
        None => Err(AppError::DatabaseNotFound {
            name: name.to_string(),
        }),
    }
}

/// 配下のどれかに Read 以上で到達できるデータベースだけを返す。
///
/// テーブル単位の権限しか持たないユーザーにもそのテーブルを含むデータベースが見えるのは、
/// 見えないと自分のテーブルへ辿り着く手段が無くなるため。
#[tracing::instrument(skip_all)]
pub async fn list(
    app_state: &AppState,
    user: &User,
) -> Result<Vec<DatabaseInfoResponse>, AppError> {
    // 絞り込みはトランザクションの外で行う。クロージャへ `user` を借用させずに済む。
    let all = app_state.db.read(async |r| r.database_list().await).await?;
    Ok(all
        .into_iter()
        .filter(|(db_id, _)| user.can(Scope::AnyIn(*db_id), UserRole::Read))
        .map(|(_, info)| info)
        .collect())
}

#[tracing::instrument(skip_all, fields(db_name = %name))]
pub async fn create(
    app_state: &AppState,
    name: &str,
    description: Option<String>,
) -> Result<DatabaseInfoResponse, AppError> {
    crate::services::helpers::name_valid::name_valid(name)?;
    if let Some(desc) = &description
        && desc.chars().count() > crate::models::database::MAX_DESCRIPTION_LENGTH
    {
        return Err(AppError::InvalidName {
            reason: format!(
                "Description cannot exceed {} characters",
                crate::models::database::MAX_DESCRIPTION_LENGTH
            ),
        });
    }

    let name = name.to_string();
    app_state
        .db
        .write(async move |w| w.database_create(&name, description).await)
        .await
}

/// 列挙と削除は [`WriteRepository::database_remove`] 側で 1 トランザクションに閉じてある。
#[tracing::instrument(skip_all, fields(db_name = %name))]
pub async fn remove(app_state: &AppState, name: &str) -> Result<(), AppError> {
    let name = name.to_string();
    app_state
        .db
        .write(async move |w| w.database_remove(&name).await)
        .await
}

#[tracing::instrument(skip_all, fields(db_name = %name))]
pub async fn update(
    app_state: &AppState,
    name: &str,
    new_name: Option<String>,
    description: Option<Option<String>>,
) -> Result<(), AppError> {
    if let Some(new_n) = &new_name {
        crate::services::helpers::name_valid::name_valid(new_n)?;
    }
    if let Some(Some(desc)) = &description
        && desc.chars().count() > crate::models::database::MAX_DESCRIPTION_LENGTH
    {
        return Err(AppError::InvalidName {
            reason: format!(
                "Description cannot exceed {} characters",
                crate::models::database::MAX_DESCRIPTION_LENGTH
            ),
        });
    }

    let name = name.to_string();
    app_state
        .db
        .write(async move |w| w.database_update(&name, new_name, description).await)
        .await
}

/// `global` の Manage 以上を要求する。そのロールは複製先にも届くので権限の付け直しは不要。
#[tracing::instrument(skip_all, fields(db_name = %name, copy_name = %copy_name))]
pub async fn copy(
    app_state: &AppState,
    name: &str,
    copy_name: &str,
) -> Result<DatabaseInfoResponse, AppError> {
    let name = name.to_string();
    let copy_name = copy_name.to_string();
    app_state
        .db
        .write(async move |w| w.database_copy(&name, &copy_name).await)
        .await
}

pub mod table;
