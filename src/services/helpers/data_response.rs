//! 「値ごとにグループ化された空間ID群」を API のレスポンス形へ整形する共通処理。
//!
//! `search`（格納値をそのまま返す）と `query`（クエリの計算結果を返す）は、値の作り方こそ
//! 違うものの出力形は同一（値辞書 + 空間ID群）なので、整形はここに一本化する。

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

/// 値ごとにグループ化された結果を、指定フォーマットのレスポンスへ整形する。
///
/// # Arguments
/// - `groups`  - `(値, その値を持つ FlexId 群)` の列
/// - `format`  - 空間IDの出力形式
/// - `limit`   - 出力する空間IDの上限（`None` なら無制限）
/// - `to_json` - 値を JSON へ変換する関数。格納バイト列の復元や数値変換はここで行う
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
                // 値グループ全体を SpatialIdSet へまとめ、range_ids_in で（空間はそのSegmentの
                // 自然な粒度を保ったまま）時間だけ暦の単位（AllowedIntervals::calendar）に
                // 結合し直してから、各領域を single_ids で展開する。
                //
                // flat_single_ids_in は使わない：あちらは Set 全体の最大ズームへ空間側も
                // 強制的に均してしまい、本来1エントリで表せる粗いブロックまで最深セル単位へ
                // 分解してしまう（例: 917件で済むはずが4242件に膨れる）。
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
                    out.push(RawRangeId {
                        z: range_id.z(),
                        f: range_id.f(),
                        x: range_id.x(),
                        y: range_id.y(),
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
/// フォーマット差は `expand`（値グループの `FlexId` 群を出力ID列へ変換しつつ上限を消費する）
/// だけに閉じ込め、辞書付番・上限判定・空グループの除去はここで共通化する。
///
/// `expand` はグループ全体（`Vec<FlexId>`）を受け取る。`SingleId`/`RangeId` は時間方向の
/// 結合（coalescing）に値グループ全体が必要なため、`FlexId` 単位の処理はできない。
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

        // 辞書へ載せるのは、実際に出力される空間IDを持つ値だけ。
        // 先に push すると、`limit` を使い切って `data` に載らなかったグループの値が
        // どこからも参照されない辞書エントリとしてレスポンスに残ってしまう。
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
