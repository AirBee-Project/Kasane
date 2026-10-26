use crate::Kasane;
use crate::error::AppError;
use crate::models::table::{CreateTableRequest, TableConstraints, TableInfo};
use crate::models::users::{Scope, User, UserRole};
use crate::repositories::{CatalogRepository, ReadRepository, Storage, WriteRepository};
use crate::services::auth::{authorize_path, reaches, visible_database};
use crate::services::helpers::authorize::resolve_authorized_table;

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn create(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
    req: CreateTableRequest,
) -> Result<TableInfo, AppError> {
    crate::services::helpers::name_valid::name_valid(table_name)?;
    if let Some(desc) = &req.description
        && desc.chars().count() > crate::models::database::MAX_DESCRIPTION_LENGTH
    {
        return Err(AppError::InvalidArgument {
            reason: format!(
                "Description cannot exceed {} characters",
                crate::models::database::MAX_DESCRIPTION_LENGTH
            ),
        });
    }

    spatial_id::ZoomLevel::new(req.max_zoom_level)?;

    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let user = user.clone();

    let table = kasane
        .db
        .write(async move |db| {
            authorize_path(db, &user, &db_name, None, UserRole::Manage).await?;
            db.table_create(
                &db_name,
                &table_name,
                req.data_type,
                req.max_zoom_level,
                req.constraints.clone(),
                req.description.clone(),
                req.is_temporal,
            )
            .await
        })
        .await?;
    Ok(TableInfo { table, count: 0 })
}

/// 認可・解決・件数取得を 1 回の読み取りで済ませる。
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn info(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
) -> Result<TableInfo, AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let user = user.clone();

    kasane
        .db
        .read(async move |db| {
            let table =
                resolve_authorized_table(db, &user, &db_name, &table_name, UserRole::Read).await?;
            let count = db.table_count(table.id).await?;
            Ok(TableInfo { table, count })
        })
        .await
}

/// テーブル一覧を返す。呼び出したユーザーが Read 以上で到達できるテーブルだけに絞る。
#[tracing::instrument(skip_all, fields(db_name = %db_name))]
pub async fn list(kasane: &Kasane, user: &User, db_name: &str) -> Result<Vec<TableInfo>, AppError> {
    let owned = db_name.to_string();
    let user = user.clone();

    kasane
        .db
        .read(async move |db| {
            let found = db.database_id(&owned).await?.map(|db_id| (db_id, ()));
            let (db_id, ()) = visible_database(db, &user, &owned, found).await?;

            let tables = if reaches(db, &user, Scope::Database(db_id), UserRole::Read).await? {
                db.table_list_by_id(db_id).await?
            } else {
                let ids: Vec<_> = db
                    .acl_tables_in(user.id, db_id)
                    .await?
                    .into_iter()
                    .collect();
                db.tables_by_id(db_id, &ids).await?
            };

            let mut infos = Vec::with_capacity(tables.len());
            for table in tables {
                let count = db.table_count(table.id).await?;
                infos.push(TableInfo { table, count });
            }
            Ok(infos)
        })
        .await
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn update(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
    new_name: Option<&str>,
    new_constraints: Option<Option<TableConstraints>>,
    description: Option<Option<String>>,
    is_temporal: Option<bool>,
) -> Result<TableInfo, AppError> {
    if let Some(Some(desc)) = &description
        && desc.chars().count() > crate::models::database::MAX_DESCRIPTION_LENGTH
    {
        return Err(AppError::InvalidArgument {
            reason: format!(
                "Description cannot exceed {} characters",
                crate::models::database::MAX_DESCRIPTION_LENGTH
            ),
        });
    }

    if is_temporal == Some(false) {
        return Err(AppError::ConstraintViolation {
            reason: "is_temporal cannot be set to false once a table has been created \
                (there is no cheap way to verify no temporal data already exists)"
                .to_string(),
        });
    }

    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let new_name = new_name.map(str::to_string);
    let user = user.clone();

    kasane
        .db
        .write(async move |txn| {
            // 権限はテーブル ID に紐づくので、改名しても権限はそのテーブルに追従する。
            authorize_path(txn, &user, &db_name, Some(&table_name), UserRole::Manage).await?;
            let table = txn
                .table_update(
                    &db_name,
                    &table_name,
                    new_name.as_deref(),
                    new_constraints.clone(),
                    description.clone(),
                    is_temporal,
                )
                .await?;
            let count = txn.table_count(table.id).await?;
            Ok(TableInfo { table, count })
        })
        .await
}

/// 認可を書き込みトランザクションの中で行う。
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn remove(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
) -> Result<(), AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let user = user.clone();

    kasane
        .db
        .write(async move |db| {
            authorize_path(db, &user, &db_name, Some(&table_name), UserRole::Manage).await?;
            db.table_remove(&db_name, &table_name).await
        })
        .await
}
