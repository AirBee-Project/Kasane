use crate::{
    AppState,
    error::AppError,
    models::{
        database::table::data::{
            GetDataQuery, OutputFormat, StreamDataEventFlex, StreamDataEventRange,
            StreamDataEventSingle, StreamDictionaryEvent, StreamEventFlex, StreamEventRange,
            StreamEventSingle, ZoomLevelPolicy,
        },
        spatial_id::{RawFlexId, RawRangeId, RawSingleId, SpatialId},
    },
    repositories::KasaneDbRead,
    services::helpers::{spatial_ids::process_spatial_ids, value::restore_value},
};
use kasane_logic::{FlexId, IntoSingleIds, RangeId};
use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub async fn get_stream(
    app_state: &AppState,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    zoom_level_policy: &ZoomLevelPolicy,
    query: &GetDataQuery,
) -> Result<impl tokio_stream::Stream<Item = Result<String, AppError>> + use<>, AppError> {
    let read_txn = app_state.db.env.read_txn()?;
    let db = KasaneDbRead::new(read_txn, &app_state.db);
    let table = match db.table_info(db_name, table_name) {
        Ok(Some(v)) => v,
        Ok(None) => {
            tracing::debug!("Table not found: {}", table_name);
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
        Err(e) => {
            tracing::error!("Failed to get table info for '{}': {}", table_name, e);
            return Err(e);
        }
    };
    let ids = process_spatial_ids(spatial_ids, table.max_zoom_level, zoom_level_policy)?;
    tracing::debug!("Searching {} spatial IDs for streaming", ids.count());

    let data_type = table.data_type;

    let (sender, mut receiver) = mpsc::channel::<Result<(Vec<u8>, Vec<FlexId>), AppError>>(100);
    db.data_get_stream(table.id, ids, sender);

    let format = query.format;
    let mut limit_left = query.limit;
    let mut sent_hashes: HashSet<u64> = HashSet::new();

    let (out_sender, out_receiver) = mpsc::channel::<Result<String, AppError>>(100);

    tokio::spawn(async move {
        while let Some(res) = receiver.recv().await {
            if let Some(left) = limit_left
                && left == 0
            {
                break;
            }

            match res {
                Ok((bytes, flex_ids)) => {
                    let mut hasher = DefaultHasher::new();
                    bytes.hash(&mut hasher);
                    let hash_val = hasher.finish();
                    let value_ref = format!("{:x}", hash_val);

                    if !sent_hashes.contains(&hash_val) {
                        sent_hashes.insert(hash_val);
                        let json_value = match restore_value(data_type, &bytes) {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = out_sender.send(Err(e)).await;
                                break;
                            }
                        };
                        let dict_event = StreamDictionaryEvent {
                            value_ref: value_ref.clone(),
                            value: json_value,
                        };

                        let ndjson = match format {
                            OutputFormat::SingleId => {
                                serde_json::to_string(&StreamEventSingle::Dictionary(dict_event))
                            }
                            OutputFormat::RangeId => {
                                serde_json::to_string(&StreamEventRange::Dictionary(dict_event))
                            }
                            OutputFormat::FlexId => {
                                serde_json::to_string(&StreamEventFlex::Dictionary(dict_event))
                            }
                        };

                        if let Ok(ndjson_str) = ndjson
                            && out_sender.send(Ok(ndjson_str + "\n")).await.is_err()
                        {
                            break;
                        }
                    }

                    // Then handle spatial_ids
                    match format {
                        OutputFormat::SingleId => {
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
                                let ev = StreamEventSingle::Data(StreamDataEventSingle {
                                    value_ref,
                                    spatial_ids,
                                });
                                let str = serde_json::to_string(&ev).unwrap() + "\n";
                                if out_sender.send(Ok(str)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        OutputFormat::RangeId => {
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
                                let ev = StreamEventRange::Data(StreamDataEventRange {
                                    value_ref,
                                    spatial_ids,
                                });
                                let str = serde_json::to_string(&ev).unwrap() + "\n";
                                if out_sender.send(Ok(str)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        OutputFormat::FlexId => {
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
                                let ev = StreamEventFlex::Data(StreamDataEventFlex {
                                    value_ref,
                                    spatial_ids,
                                });
                                let str = serde_json::to_string(&ev).unwrap() + "\n";
                                if out_sender.send(Ok(str)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = out_sender.send(Err(e)).await;
                    break;
                }
            }
        }
    });

    Ok(ReceiverStream::new(out_receiver))
}
