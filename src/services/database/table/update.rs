use crate::{
    AppState, error::AppError, models::database::table::TableInfoResponse,
    repositories::KasaneDbWrite,
};

pub async fn table_update(
    state: AppState,
    db_name: &str,
    table_name: &str,
    new_name: Option<&str>,
    new_constraints: Option<Option<crate::models::database::table::UpdateTableConstraints>>,
    validate_existing_data: bool,
) -> Result<TableInfoResponse, AppError> {
    let mut txn = KasaneDbWrite::new(
        state.db.env.write_txn().map_err(AppError::StorageError)?,
        &state.db,
    );

    let table = txn.table_update(
        db_name,
        table_name,
        new_name,
        new_constraints,
        validate_existing_data,
    )?;

    // info() equivalent logic to get count
    let prefix = table.id.into_bytes();
    let tables_data = txn
        .db
        .tables_data
        .remap_types::<heed::types::Bytes, heed::types::Bytes>();

    // I need to count the exact number of elements inside the chunks as info.rs does.
    // Let me just copy the logic from info.rs.
    let count: u64 = {
        let mut total = 0;
        for iter in tables_data.prefix_iter(&txn.write_txn, prefix.as_slice())? {
            let (_, v_bytes) = iter?;
            use crate::repositories::database::table::data::shard::ShardEntry;
            if let Some(c) = ShardEntry::leaf_count(v_bytes)? {
                total += c as u64;
            }
        }
        total
    };

    txn.commit()?;

    Ok(TableInfoResponse {
        name: table.name,
        data_type: table.data_type,
        max_zoom_level: table.max_zoom_level,
        constraints: table.constraints,
        count,
    })
}
