use crate::{
    AppState,
    error::AppError,
    middleware::auth::{authorize_path, visible_database},
    models::database::DatabaseInfoResponse,
    models::users::{User, UserRole},
    repositories::{CatalogRepository, ReadRepository, Storage, WriteRepository},
};

/// 認可と取得を 1 回の読み取りにまとめる。本体は 1 度しか引かない。
#[tracing::instrument(skip_all, fields(db_name = %name))]
pub async fn info(
    app_state: &AppState,
    user: &User,
    name: &str,
) -> Result<DatabaseInfoResponse, AppError> {
    let owned = name.to_string();
    let user = user.clone();
    app_state
        .db
        .read(async move |r| {
            let found = r.database_info(&owned).await?;
            Ok(visible_database(r, &user, &owned, found).await?.1)
        })
        .await
}

/// 配下のどれかに Read 以上で到達できるデータベースだけを返す。
///
/// テーブル単位の権限しか持たないユーザーにもそのテーブルを含むデータベースが見えるのは、
/// 見えないと自分のテーブルへ辿り着く手段が無くなるため。
///
/// **絞り込みは権限の側から行う。** 全データベースを列挙してから捨てる形だと、権限を
/// 1 つも持たない対象の読み取りとフィルタ計算まで払うことになり、データベース数に
/// 比例して重くなる。全体ロールを持つ利用者だけは全件が対象なので、そちらは列挙する。
#[tracing::instrument(skip_all)]
pub async fn list(
    app_state: &AppState,
    user: &User,
) -> Result<Vec<DatabaseInfoResponse>, AppError> {
    let user = user.clone();
    app_state
        .db
        .read(async move |r| {
            if user.has_global_role(UserRole::Read) {
                return Ok(r
                    .database_list()
                    .await?
                    .into_iter()
                    .map(|(_, info)| info)
                    .collect());
            }

            let db_ids: Vec<_> = r.acl_databases(user.id).await?.into_iter().collect();
            Ok(r.databases_by_id(&db_ids)
                .await?
                .into_iter()
                .map(|(_, info)| info)
                .collect())
        })
        .await
}

#[tracing::instrument(skip_all, fields(db_name = %name))]
pub async fn create(
    app_state: &AppState,
    name: &str,
    description: Option<String>,
) -> Result<DatabaseInfoResponse, AppError> {
    crate::services::helpers::name_valid::name_valid(name)?;
    check_description(description.as_deref())?;

    let name = name.to_string();
    app_state
        .db
        .write(async move |w| w.database_create(&name, description.clone()).await)
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

/// 認可を書き込みトランザクションの中で行う。
///
/// 判定した対象と書き換える対象が同じであることが、断面が 1 つであることから従う。
#[tracing::instrument(skip_all, fields(db_name = %name))]
pub async fn update(
    app_state: &AppState,
    user: &User,
    name: &str,
    new_name: Option<String>,
    description: Option<Option<String>>,
) -> Result<(), AppError> {
    if let Some(new_n) = &new_name {
        crate::services::helpers::name_valid::name_valid(new_n)?;
    }
    if let Some(desc) = &description {
        check_description(desc.as_deref())?;
    }

    let name = name.to_string();
    let user = user.clone();
    app_state
        .db
        .write(async move |w| {
            authorize_path(w, &user, &name, None, UserRole::Manage).await?;
            w.database_update(&name, new_name.clone(), description.clone())
                .await
        })
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

fn check_description(description: Option<&str>) -> Result<(), AppError> {
    let limit = crate::models::database::MAX_DESCRIPTION_LENGTH;
    match description {
        Some(desc) if desc.chars().count() > limit => Err(AppError::InvalidName {
            reason: format!("Description cannot exceed {limit} characters"),
        }),
        _ => Ok(()),
    }
}

pub mod table;
