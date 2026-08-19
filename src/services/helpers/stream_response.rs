use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use kasane_logic::{AllowedIntervals, FlexId, SpatialId as _, SpatialIdSet};
use serde_json::Value;

use crate::{
    error::AppError,
    models::{
        database::table::data::OutputFormat,
        spatial_id::{RawFlexId, RawRangeId, RawSingleId},
    },
};

fn take_one(limit_left: &mut Option<usize>) -> bool {
    match limit_left {
        Some(0) => false,
        Some(left) => {
            *left -= 1;
            true
        }
        None => true,
    }
}

pub fn stream_json<V, I, F>(
    groups: I,
    format: OutputFormat,
    limit: Option<usize>,
    to_json: F,
) -> Result<Response, AppError>
where
    I: IntoIterator<Item = (V, Vec<FlexId>)>,
    F: Fn(&V) -> Result<Value, AppError>,
{
    let mut dictionary = Vec::new();
    let mut entries: Vec<String> = Vec::new();
    let mut limit_left = limit;

    'groups: for (value, flex_ids) in groups {
        if limit_left == Some(0) {
            break;
        }

        let spatial_ids_json = match format {
            OutputFormat::SingleId => {
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                let mut out = Vec::new();
                'ranges: for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
                    for single_id in range_id.single_ids() {
                        if !take_one(&mut limit_left) {
                            break 'ranges;
                        }
                        let (i, t) = if single_id.is_whole_time() {
                            (None, None)
                        } else {
                            (
                                Some(single_id.time_interval().seconds()),
                                Some(single_id.t()),
                            )
                        };
                        out.push(RawSingleId {
                            z: single_id.z(),
                            f: single_id.f(),
                            x: single_id.x(),
                            y: single_id.y(),
                            i,
                            t,
                        });
                    }
                }
                if out.is_empty() {
                    continue 'groups;
                }
                serde_json::to_string(&out).unwrap()
            }
            OutputFormat::RangeId => {
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                let mut out = Vec::new();
                for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
                    if !take_one(&mut limit_left) {
                        break;
                    }
                    let (i, t) = if range_id.is_whole_time() {
                        (None, None)
                    } else {
                        (Some(range_id.time_interval().seconds()), Some(range_id.t()))
                    };
                    out.push(RawRangeId {
                        z: range_id.z(),
                        f: Some(range_id.f()),
                        x: Some(range_id.x()),
                        y: Some(range_id.y()),
                        i,
                        t,
                    });
                }
                if out.is_empty() {
                    continue 'groups;
                }
                serde_json::to_string(&out).unwrap()
            }
            OutputFormat::FlexId => {
                let mut out = Vec::new();
                for flex_id in flex_ids {
                    if !take_one(&mut limit_left) {
                        break;
                    }
                    let (t_zoomlevel, t_index) = if flex_id.is_whole_time() {
                        (None, None)
                    } else {
                        (Some(flex_id.t_zoomlevel()), Some(flex_id.t()))
                    };
                    out.push(RawFlexId {
                        f_zoomlevel: flex_id.f_zoomlevel(),
                        f_index: flex_id.f_index(),
                        x_zoomlevel: flex_id.x_zoomlevel(),
                        x_index: flex_id.x_index(),
                        y_zoomlevel: flex_id.y_zoomlevel(),
                        y_index: flex_id.y_index(),
                        t_zoomlevel,
                        t_index,
                    });
                }
                if out.is_empty() {
                    continue 'groups;
                }
                serde_json::to_string(&out).unwrap()
            }
        };

        // 先に push すると、limit で data に載らなかった値が孤立した辞書項目として残る。
        let value_ref = dictionary.len();
        dictionary.push(to_json(&value)?);
        entries.push(format!(
            "{{\"valueRef\":{},\"spatialIds\":{}}}",
            value_ref, spatial_ids_json
        ));
    }

    let dict_json = serde_json::to_string(&dictionary).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, String>>(4);

    tokio::task::spawn_blocking(move || {
        let start_str = format!("{{\"dictionary\":{},\"data\":[", dict_json);
        if tx.blocking_send(Ok(Bytes::from(start_str))).is_err() {
            return;
        }

        for (i, entry) in entries.into_iter().enumerate() {
            let chunk = if i == 0 { entry } else { format!(",{}", entry) };
            if tx.blocking_send(Ok(Bytes::from(chunk))).is_err() {
                return;
            }
        }

        let _ = tx.blocking_send(Ok(Bytes::from("]}")));
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from_stream(stream))
        .unwrap()
        .into_response())
}
