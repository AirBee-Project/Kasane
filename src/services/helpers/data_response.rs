//! 「値ごとにグループ化された空間ID群」を API のレスポンス形へ整形する共通処理。
//!
//! `search`（格納値をそのまま返す）と `query`（クエリの計算結果を返す）は、値の作り方こそ
//! 違うものの出力形は同一（値辞書 + 空間ID群）なので、整形はここに一本化する。

use kasane_logic::{FlexId, RangeId, SpatialId as _};

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
            let (dictionary, data) = build_groups(groups, limit, to_json, |flex_id, left, out| {
                // SingleId は FlexId 1つが複数セルへ展開されるため、展開しながら上限を見る。
                for single_id in flex_id.single_ids() {
                    if !take_one(left) {
                        break;
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
            })?;
            GetDataResponse::Single(GetDataResponseSingle { dictionary, data })
        }
        OutputFormat::RangeId => {
            let (dictionary, data) = build_groups(groups, limit, to_json, |flex_id, left, out| {
                if !take_one(left) {
                    return;
                }
                let range_id = RangeId::from(&flex_id);
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
            })?;
            GetDataResponse::Range(GetDataResponseRange { dictionary, data })
        }
        OutputFormat::FlexId => {
            let (dictionary, data) = build_groups(groups, limit, to_json, |flex_id, left, out| {
                if !take_one(left) {
                    return;
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
            })?;
            GetDataResponse::Flex(GetDataResponseFlex { dictionary, data })
        }
    })
}

/// 値辞書と、出力ID型 `I` のデータ群を組み立てる。
///
/// フォーマット差は `expand`（`FlexId` を出力ID群へ展開しつつ上限を消費する）だけに閉じ込め、
/// 辞書付番・上限判定・空グループの除去はここで共通化する。
fn build_groups<V, I, F, E>(
    groups: impl IntoIterator<Item = (V, Vec<FlexId>)>,
    limit: Option<usize>,
    to_json: F,
    expand: E,
) -> Result<(Vec<serde_json::Value>, Vec<DataGroup<I>>), AppError>
where
    F: Fn(&V) -> Result<serde_json::Value, AppError>,
    E: Fn(FlexId, &mut Option<usize>, &mut Vec<I>),
{
    let mut dictionary = Vec::new();
    let mut data = Vec::new();
    let mut limit_left = limit;

    for (value, flex_ids) in groups {
        let mut spatial_ids = Vec::with_capacity(flex_ids.len());
        for flex_id in flex_ids {
            expand(flex_id, &mut limit_left, &mut spatial_ids);
            if limit_left == Some(0) {
                break;
            }
        }

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
