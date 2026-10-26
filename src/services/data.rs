use crate::Kasane;
use crate::error::{AppError, Resource};
use crate::models::data::{GetDataQuery, GetDataResponse, ZoomLevelPolicy};
use crate::models::spatial_id::SpatialId;
use crate::models::users::{User, UserRole};
use crate::repositories::{ReadRepository, Storage, WriteRepository};
use crate::services::helpers::authorize::authorized_table;
use crate::services::helpers::data_response;
use crate::services::helpers::spatial_ids::process_spatial_ids;
use crate::services::helpers::value::{enforce_max_stored_size, interpret_value, restore_value};

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn get(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
    query: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let spatial_ids = spatial_ids.to_vec();
    let zoom_level_policy = *zoom_level_policy;
    let query_format = query.format;
    let user = user.clone();

    let (table, groups) = kasane
        .db
        .read(async move |db| {
            let refs = [(db_name.clone(), table_name.clone())];
            let resolved = db.resolve_tables(&refs).await?.pop().ok_or_else(|| {
                AppError::Internal("resolve_tables dropped its only request".to_string())
            })?;

            crate::services::auth::authorize_resolved(
                db,
                &user,
                resolved.db_id,
                resolved.table.as_ref().map(|t| t.id),
                &db_name,
                Some(&table_name),
                UserRole::Read,
            )
            .await?;

            let table = resolved.table.ok_or_else(|| {
                tracing::debug!("Table not found: {}", table_name);
                Resource::Table.not_found(table_name.clone())
            })?;

            let ids = process_spatial_ids(
                &spatial_ids,
                table.max_zoom_level,
                &zoom_level_policy,
                false,
            )?;
            let groups = db.data_get(table.id, ids).await?;
            Ok((table, groups))
        })
        .await?;

    data_response::build(groups, query_format, |bytes| {
        restore_value(table.data_type, table.constraints.as_ref(), bytes)
    })
}

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn insert(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    value: crate::models::ValueLiteral,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let table = authorized_table(kasane, user, db_name, table_name, UserRole::Write).await?;

    let value = interpret_value(table.data_type, table.constraints.as_ref(), value)?;
    enforce_max_stored_size(&value)?;
    let ids = process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        zoom_level_policy,
        !table.is_temporal,
    )?;

    kasane
        .db
        .write(async move |w| w.data_insert(table.id, ids.clone(), &value).await)
        .await
}

/// 値が存在しないIDにのみ書き込む（Upsert）
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn upsert(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    value: crate::models::ValueLiteral,
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let table = authorized_table(kasane, user, db_name, table_name, UserRole::Write).await?;

    let value = interpret_value(table.data_type, table.constraints.as_ref(), value)?;
    enforce_max_stored_size(&value)?;
    let ids = process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        zoom_level_policy,
        !table.is_temporal,
    )?;

    kasane
        .db
        .write(async move |w| w.data_upsert(table.id, ids.clone(), &value).await)
        .await
}

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn remove(
    kasane: &Kasane,
    user: &User,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let table = authorized_table(kasane, user, db_name, table_name, UserRole::Write).await?;

    let ids = process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        zoom_level_policy,
        !table.is_temporal,
    )?;

    kasane
        .db
        .write(async move |w| w.data_remove(table.id, ids.clone()).await)
        .await
}
