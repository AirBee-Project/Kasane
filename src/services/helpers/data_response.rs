//! 「値ごとにグループ化された空間ID群」を API のレスポンス形へ整形する。
//!
//! `search` と `query` は値の作り方こそ違うが出力形は同一なので、整形を一本化する。

use kasane_logic::{AllowedIntervals, FlexId, SpatialIdSet};

use crate::{
    error::AppError,
    models::database::table::data::{
        DataGroup, GetDataResponse, GetDataResponseFlex, GetDataResponseRange,
        GetDataResponseSingle, OutputFormat,
    },
};

use crate::models::ValueLiteral;

/// `to_value` は値を [`ValueLiteral`] へ変換する。格納バイト列の復元や数値変換はそこで行う。
pub fn build<V, F>(
    groups: impl IntoIterator<Item = (V, Vec<FlexId>)>,
    format: OutputFormat,
    to_value: F,
) -> Result<GetDataResponse, AppError>
where
    F: Fn(&V) -> Result<ValueLiteral, AppError>,
{
    Ok(match format {
        OutputFormat::SingleId => {
            let (dictionary, data) = build_groups(groups, to_value, |flex_ids| {
                // `flat_single_ids_in` は空間側も最大ズームへ均す（917 件 → 4242 件）。
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                set.range_ids_in(AllowedIntervals::calendar())
                    .flat_map(|range_id| range_id.single_ids())
                    .collect()
            })?;
            GetDataResponse::Single(GetDataResponseSingle { dictionary, data })
        }
        OutputFormat::RangeId => {
            let (dictionary, data) = build_groups(groups, to_value, |flex_ids| {
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                set.range_ids_in(AllowedIntervals::calendar()).collect()
            })?;
            GetDataResponse::Range(GetDataResponseRange { dictionary, data })
        }
        OutputFormat::FlexId => {
            let (dictionary, data) = build_groups(groups, to_value, |flex_ids| flex_ids)?;
            GetDataResponse::Flex(GetDataResponseFlex { dictionary, data })
        }
    })
}

/// 値辞書と、出力ID型 `I` のデータ群を組み立てる。
///
/// フォーマット差は `expand` だけに閉じ込め、辞書付番・空グループの除去を共通化する。
fn build_groups<V, I, F, E>(
    groups: impl IntoIterator<Item = (V, Vec<FlexId>)>,
    to_value: F,
    expand: E,
) -> Result<(Vec<ValueLiteral>, Vec<DataGroup<I>>), AppError>
where
    F: Fn(&V) -> Result<ValueLiteral, AppError>,
    E: Fn(Vec<FlexId>) -> Vec<I>,
{
    let mut dictionary = Vec::new();
    let mut data = Vec::new();

    for (value, flex_ids) in groups {
        let spatial_ids = expand(flex_ids);

        if !spatial_ids.is_empty() {
            let value_ref = dictionary.len();
            dictionary.push(to_value(&value)?);
            data.push(DataGroup {
                value_ref,
                spatial_ids,
            });
        }
    }

    Ok((dictionary, data))
}
