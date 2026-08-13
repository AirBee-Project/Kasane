//! 「値ごとにグループ化された空間ID群」を API のレスポンス形へ整形する。
//!
//! `search` と `query` は値の作り方こそ違うが出力形は同一なので、整形を一本化する。

use kasane_logic::{AllowedIntervals, FlexId, SpatialId as _, SpatialIdSet};

use crate::{
    error::AppError,
    models::{
        database::table::data::{
            DataGroup, GetDataResponse, GetDataResponseFlex, GetDataResponseRange,
            GetDataResponseSingle, OutputFormat,
        },
        spatial_id::{RawFlexId, RawRangeId, RawSingleId},
    },
};

/// `to_json` は値を JSON へ変換する。格納バイト列の復元や数値変換はそこで行う。
pub fn build<V, F>(
    groups: impl IntoIterator<Item = (V, Vec<FlexId>)>,
    format: OutputFormat,
    limit: Option<usize>,
    to_json: F,
) -> Result<GetDataResponse, AppError>
where
    F: Fn(&V) -> Result<serde_json::Value, AppError>,
{
    Ok(match format {
        OutputFormat::SingleId => {
            let (dictionary, data) = build_groups(groups, limit, to_json, |flex_ids, left| {
                // `flat_single_ids_in` は空間側も最大ズームへ均す（917 件 → 4242 件）。
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                let mut out = Vec::new();
                'ranges: for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
                    for single_id in range_id.single_ids() {
                        if !take_one(left) {
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
                out
            })?;
            GetDataResponse::Single(GetDataResponseSingle { dictionary, data })
        }
        OutputFormat::RangeId => {
            let (dictionary, data) = build_groups(groups, limit, to_json, |flex_ids, left| {
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                let mut out = Vec::new();
                for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
                    if !take_one(left) {
                        break;
                    }
                    let (i, t) = if range_id.is_whole_time() {
                        (None, None)
                    } else {
                        (
                            Some(range_id.time_interval().seconds()),
                            Some(range_id.t()), // RangeId returns [u64; 2] for t()
                        )
                    };
                    // 出力では省略記法を使わず、常に具体的な範囲を書き出す。
                    out.push(RawRangeId {
                        z: range_id.z(),
                        f: Some(range_id.f()),
                        x: Some(range_id.x()),
                        y: Some(range_id.y()),
                        i,
                        t,
                    });
                }
                out
            })?;
            GetDataResponse::Range(GetDataResponseRange { dictionary, data })
        }
        OutputFormat::FlexId => {
            let (dictionary, data) = build_groups(groups, limit, to_json, |flex_ids, left| {
                let mut out = Vec::new();
                for flex_id in flex_ids {
                    if !take_one(left) {
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
                out
            })?;
            GetDataResponse::Flex(GetDataResponseFlex { dictionary, data })
        }
    })
}

/// 値辞書と、出力ID型 `I` のデータ群を組み立てる。
///
/// フォーマット差は `expand` だけに閉じ込め、辞書付番・上限判定・空グループの除去を共通化する。
/// `expand` がグループ全体を受け取るのは、`SingleId`/`RangeId` の時間方向の結合に値グループ
/// 全体が要るため。
fn build_groups<V, I, F, E>(
    groups: impl IntoIterator<Item = (V, Vec<FlexId>)>,
    limit: Option<usize>,
    to_json: F,
    expand: E,
) -> Result<(Vec<serde_json::Value>, Vec<DataGroup<I>>), AppError>
where
    F: Fn(&V) -> Result<serde_json::Value, AppError>,
    E: Fn(Vec<FlexId>, &mut Option<usize>) -> Vec<I>,
{
    let mut dictionary = Vec::new();
    let mut data = Vec::new();
    let mut limit_left = limit;

    for (value, flex_ids) in groups {
        let spatial_ids = expand(flex_ids, &mut limit_left);

        // 先に push すると、`limit` で `data` に載らなかった値が孤立した辞書項目として残る。
        if !spatial_ids.is_empty() {
            let value_ref = dictionary.len();
            dictionary.push(to_json(&value)?);
            data.push(DataGroup {
                value_ref,
                spatial_ids,
            });
        }
        if limit_left == Some(0) {
            break;
        }
    }

    Ok((dictionary, data))
}

/// 残り件数を1つ消費する。使い切っていれば `false`。
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
