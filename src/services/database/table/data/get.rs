use crate::{
    AppState,
    error::AppError,
    models::{
        database::table::data::{
            DataGroup, GetDataQuery, GetDataResponse, GetDataResponseFlex, GetDataResponseRange,
            GetDataResponseSingle, OutputFormat, ZoomLevelPolicy,
        },
        spatial_id::{RawFlexId, RawRangeId, RawSingleId, SpatialId},
    },
    repositories::KasaneDbRead,
    services::helpers::{spatial_ids::process_spatial_ids, value::restore_value},
};
use kasane_logic::{IntoSingleIds, RangeId};

pub async fn get(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
    query: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    let app_state = app_state.clone();
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let spatial_ids = spatial_ids.to_vec();
    let zoom_level_policy = *zoom_level_policy;
    let query_format = query.format;
    let query_limit = query.limit;

    tokio::task::spawn_blocking(move || {
        let read_txn = app_state.db.env.read_txn()?;
        let db = KasaneDbRead::new(read_txn, &app_state.db);
        let table = match db.table_info(&db_name, &table_name) {
            Ok(Some(v)) => v,
            Ok(None) => {
                tracing::debug!("Table not found: {}", table_name);
                return Err(AppError::TableNotFound {
                    name: table_name.clone(),
                });
            }
            Err(e) => {
                tracing::error!("Failed to get table info for '{}': {}", table_name, e);
                return Err(e);
            }
        };
        let ids = process_spatial_ids(&spatial_ids, table.max_zoom_level, &zoom_level_policy)?;
        tracing::debug!("Searching {} spatial IDs", ids.count());

        let data_type = table.data_type;
        let groups = db.data_get(table.id, ids)?;

        let mut dictionary = Vec::new();
        let mut limit_left = query_limit;

        match query_format {
            OutputFormat::SingleId => {
                let mut data = Vec::new();
                for (bytes, flex_ids) in groups {
                    let json_value = restore_value(data_type, &bytes)?;
                    let value_ref = dictionary.len();
                    dictionary.push(json_value);

                    let mut spatial_ids = Vec::new();
                    for flex_id in flex_ids {
                        for single_id in flex_id.into_single_ids() {
                            if let Some(left) = limit_left.as_mut() {
                                if *left == 0 {
                                    break;
                                }
                                *left -= 1;
                            }
                            spatial_ids.push(RawSingleId {
                                z: single_id.z(),
                                f: single_id.f(),
                                x: single_id.x(),
                                y: single_id.y(),
                            });
                        }
                    }
                    if !spatial_ids.is_empty() {
                        data.push(DataGroup {
                            value_ref,
                            spatial_ids,
                        });
                    }
                    if limit_left == Some(0) {
                        break;
                    }
                }
                Ok(GetDataResponse::Single(GetDataResponseSingle {
                    dictionary,
                    data,
                }))
            }
            OutputFormat::RangeId => {
                let mut data = Vec::new();
                for (bytes, flex_ids) in groups {
                    let json_value = restore_value(data_type, &bytes)?;
                    let value_ref = dictionary.len();
                    dictionary.push(json_value);

                    let mut spatial_ids = Vec::new();
                    for flex_id in flex_ids {
                        if let Some(left) = limit_left.as_mut() {
                            if *left == 0 {
                                break;
                            }
                            *left -= 1;
                        }
                        let range_id = RangeId::from(&flex_id);
                        spatial_ids.push(RawRangeId {
                            z: range_id.z(),
                            f: range_id.f(),
                            x: range_id.x(),
                            y: range_id.y(),
                        });
                    }
                    if !spatial_ids.is_empty() {
                        data.push(DataGroup {
                            value_ref,
                            spatial_ids,
                        });
                    }
                    if limit_left == Some(0) {
                        break;
                    }
                }
                Ok(GetDataResponse::Range(GetDataResponseRange {
                    dictionary,
                    data,
                }))
            }
            OutputFormat::FlexId => {
                let mut data = Vec::new();
                for (bytes, flex_ids) in groups {
                    let json_value = restore_value(data_type, &bytes)?;
                    let value_ref = dictionary.len();
                    dictionary.push(json_value);

                    let mut spatial_ids = Vec::new();
                    for flex_id in flex_ids {
                        if let Some(left) = limit_left.as_mut() {
                            if *left == 0 {
                                break;
                            }
                            *left -= 1;
                        }
                        spatial_ids.push(RawFlexId {
                            f_zoomlevel: flex_id.f_zoomlevel(),
                            f_index: flex_id.f_index(),
                            x_zoomlevel: flex_id.x_zoomlevel(),
                            x_index: flex_id.x_index(),
                            y_zoomlevel: flex_id.y_zoomlevel(),
                            y_index: flex_id.y_index(),
                        });
                    }
                    if !spatial_ids.is_empty() {
                        data.push(DataGroup {
                            value_ref,
                            spatial_ids,
                        });
                    }
                    if limit_left == Some(0) {
                        break;
                    }
                }
                Ok(GetDataResponse::Flex(GetDataResponseFlex {
                    dictionary,
                    data,
                }))
            }
        }
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}
