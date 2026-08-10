use crate::{
    AppState,
    error::AppError,
    models::database::DatabaseInfoResponse,
    models::users::{Scope, User, UserRole},
    repositories::{ReadRepository, Storage, WriteRepository},
};

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

/// データベース一覧を取得する。
///
/// 配下のどれかに Read 以上で到達できるデータベースだけを返す。テーブル単位の権限しか
/// 持たないユーザーにも、そのテーブルを含むデータベースは見える（見えないと自分の
/// テーブルへ辿り着く手段が無くなるため）。
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

/// データベースを配下のテーブルごと削除する。
///
/// 列挙と削除の分割は [`WriteRepository::database_remove`] 側で 1 つの書き込み
/// トランザクションに閉じてある。
pub async fn remove(app_state: &AppState, name: &str) -> Result<(), AppError> {
    let name = name.to_string();
    app_state
        .db
        .write(async move |w| w.database_remove(&name).await)
        .await
}

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

/// データベースを複製する。
///
/// この操作は `global` スコープの Manage 以上を要求する。そのロールは複製先を含む
/// すべてのデータベースに届くので、呼び出し元へ個別に権限を付け直す必要はない。
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
