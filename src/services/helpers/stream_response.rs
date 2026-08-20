use axum::response::Response;
use bytes::Bytes;
use kasane_logic::{AllowedIntervals, FlexId, SpatialId as _, SpatialIdSet};
use serde_json::Value;

use crate::{
    error::AppError,
    models::database::table::{
        TableDataType,
        data::{OutputFormat, stream::build_stream_response},
    },
    models::spatial_id::{RawFlexId, RawRangeId, RawSingleId},
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

/// 1 group を limit 予算内で展開し、生成できた行ごとに `on_row` を呼ぶ。返り値は生成できた
/// 行数。pass1（カウントのみ）と pass2（整形して送信）の両方がこの関数を通ることで、
/// どの group が採用されるかの判定が2パスの間で必ず一致するようにする。
fn expand_single(
    flex_ids: &[FlexId],
    limit_left: &mut Option<usize>,
    mut on_row: impl FnMut(RawSingleId),
) -> usize {
    let set: SpatialIdSet = flex_ids.iter().copied().collect();
    let mut count = 0;
    'ranges: for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
        for single_id in range_id.single_ids() {
            if !take_one(limit_left) {
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
            on_row(RawSingleId {
                z: single_id.z(),
                f: single_id.f(),
                x: single_id.x(),
                y: single_id.y(),
                i,
                t,
            });
            count += 1;
        }
    }
    count
}

fn expand_range(
    flex_ids: &[FlexId],
    limit_left: &mut Option<usize>,
    mut on_row: impl FnMut(RawRangeId),
) -> usize {
    let set: SpatialIdSet = flex_ids.iter().copied().collect();
    let mut count = 0;
    for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
        if !take_one(limit_left) {
            break;
        }
        let (i, t) = if range_id.is_whole_time() {
            (None, None)
        } else {
            (Some(range_id.time_interval().seconds()), Some(range_id.t()))
        };
        on_row(RawRangeId {
            z: range_id.z(),
            f: Some(range_id.f()),
            x: Some(range_id.x()),
            y: Some(range_id.y()),
            i,
            t,
        });
        count += 1;
    }
    count
}

fn expand_flex(
    flex_ids: &[FlexId],
    limit_left: &mut Option<usize>,
    mut on_row: impl FnMut(RawFlexId),
) -> usize {
    let mut count = 0;
    for flex_id in flex_ids.iter() {
        if !take_one(limit_left) {
            break;
        }
        let (t_zoomlevel, t_index) = if flex_id.is_whole_time() {
            (None, None)
        } else {
            (Some(flex_id.t_zoomlevel()), Some(flex_id.t()))
        };
        on_row(RawFlexId {
            f_zoomlevel: flex_id.f_zoomlevel(),
            f_index: flex_id.f_index(),
            x_zoomlevel: flex_id.x_zoomlevel(),
            x_index: flex_id.x_index(),
            y_zoomlevel: flex_id.y_zoomlevel(),
            y_index: flex_id.y_index(),
            t_zoomlevel,
            t_index,
        });
        count += 1;
    }
    count
}

/// group が 1 行でも寄与するかどうかだけを、行の JSON 化なしに判定する（pass1 用）。
fn group_row_count(
    format: OutputFormat,
    flex_ids: &[FlexId],
    limit_left: &mut Option<usize>,
) -> usize {
    match format {
        OutputFormat::SingleId => expand_single(flex_ids, limit_left, |_| {}),
        OutputFormat::RangeId => expand_range(flex_ids, limit_left, |_| {}),
        OutputFormat::FlexId => expand_flex(flex_ids, limit_left, |_| {}),
    }
}

/// group を実際に整形する（pass2 用）。1 行も生成できなければ `None`
/// （呼び出し側は辞書に載せない）。
fn group_json(
    format: OutputFormat,
    flex_ids: &[FlexId],
    limit_left: &mut Option<usize>,
) -> Option<String> {
    match format {
        OutputFormat::SingleId => {
            let mut out = Vec::new();
            expand_single(flex_ids, limit_left, |row| out.push(row));
            if out.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&out).unwrap())
            }
        }
        OutputFormat::RangeId => {
            let mut out = Vec::new();
            expand_range(flex_ids, limit_left, |row| out.push(row));
            if out.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&out).unwrap())
            }
        }
        OutputFormat::FlexId => {
            let mut out = Vec::new();
            expand_flex(flex_ids, limit_left, |row| out.push(row));
            if out.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&out).unwrap())
            }
        }
    }
}

/// `{"dictionary":[...],"data":[...]}` を2パスで生成する。
///
/// pass1（この関数の呼び出し元スレッド上、まだ 1 バイトも送信していない段階）で、どの
/// group が寄与するかを確定して辞書全体を組み立てる — `to_json` はここでしか呼ばない
/// ので、失敗すれば通常のエラー応答としてそのまま返せる。
/// pass2（`spawn_blocking` 上）で同じ順序・同じ limit 予算を歩き直し、寄与する group だけを
/// その場で整形してチャンネルへ送る。全 group の整形済み文字列を溜め込むことはない。
pub fn stream_json<V, F>(
    groups: Vec<(V, Vec<FlexId>)>,
    format: OutputFormat,
    limit: Option<usize>,
    to_json: F,
) -> Result<Response, AppError>
where
    V: Send + 'static,
    F: Fn(&V) -> Result<Value, AppError>,
{
    let mut dictionary: Vec<Value> = Vec::new();
    let mut limit_left = limit;
    for (value, flex_ids) in &groups {
        if limit_left == Some(0) {
            break;
        }
        if group_row_count(format, flex_ids, &mut limit_left) == 0 {
            continue;
        }
        dictionary.push(to_json(value)?);
    }

    let dict_json = serde_json::to_string(&dictionary).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, String>>(4);

    tokio::task::spawn_blocking(move || {
        let start_str = format!("{{\"dictionary\":{},\"data\":[", dict_json);
        if tx.blocking_send(Ok(Bytes::from(start_str))).is_err() {
            return;
        }

        let mut limit_left = limit;
        let mut value_ref = 0usize;
        let mut first = true;
        for (_, flex_ids) in &groups {
            if limit_left == Some(0) {
                break;
            }
            let Some(spatial_ids_json) = group_json(format, flex_ids, &mut limit_left) else {
                continue;
            };
            let chunk = format!(
                "{}{{\"valueRef\":{},\"spatialIds\":{}}}",
                if first { "" } else { "," },
                value_ref,
                spatial_ids_json
            );
            first = false;
            value_ref += 1;
            if tx.blocking_send(Ok(Bytes::from(chunk))).is_err() {
                return;
            }
        }

        let _ = tx.blocking_send(Ok(Bytes::from("]}")));
    });

    Ok(build_stream_response(rx, "application/json"))
}

/// `is_arrow` に応じて Arrow IPC / JSON いずれかのストリーミング応答を組み立てる。
/// 呼び出し側3箇所（空 bounds の早期 return / 通常のクエリ実行 / `data_get`）で同じ分岐が
/// 重複していたのをここへ集約する。
pub fn respond<V, F>(
    groups: Vec<(V, Vec<FlexId>)>,
    format: OutputFormat,
    limit: Option<usize>,
    value_type: TableDataType,
    is_arrow: bool,
    to_json: F,
) -> Result<Response, AppError>
where
    V: Send + 'static,
    F: Fn(&V) -> Result<Value, AppError> + Send + 'static,
{
    if is_arrow {
        crate::models::database::table::data::arrow::stream_arrow_ipc(
            groups, format, limit, value_type, to_json,
        )
    } else {
        stream_json(groups, format, limit, to_json)
    }
}
