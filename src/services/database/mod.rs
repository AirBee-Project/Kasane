use crate::{
    AppState,
    error::AppError,
    models::database::DatabaseInfoResponse,
    models::users::{Scope, User, UserRole},
};

pub async fn info(app_state: &AppState, name: &str) -> Result<DatabaseInfoResponse, AppError> {
    match app_state.db.read(|r| r.database_info(name))? {
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
    app_state.db.read(|r| {
        Ok(r.database_list()?
            .into_iter()
            .filter(|(db_id, _)| user.can(Scope::AnyIn(*db_id), UserRole::Read))
            .map(|(_, info)| info)
            .collect())
    })
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

    let app_state = app_state.clone();
    let name = name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| {
            app_state
                .db
                .write(|db| db.database_create(&name, description))
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

/// データベースを配下のテーブルごと削除する。
///
/// 列挙と削除の分割は [`KasaneDbWrite::database_remove`](crate::repositories::KasaneDbWrite)
/// 側で 1 つの書き込みトランザクションに閉じてある。
pub async fn remove(app_state: &AppState, name: &str) -> Result<(), AppError> {
    let app_state = app_state.clone();
    let name = name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| app_state.db.write(|db| db.database_remove(&name)))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
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

    let app_state = app_state.clone();
    let name = name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| {
            app_state
                .db
                .write(|db| db.database_update(&name, new_name, description))
        })
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
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
    let app_state = app_state.clone();
    let name = name.to_string();
    let copy_name = copy_name.to_string();

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        span.in_scope(|| app_state.db.write(|db| db.database_copy(&name, &copy_name)))
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

pub mod table;
