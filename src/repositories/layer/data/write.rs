use kasane_logic::{IntoSingleIds, SpatialIdSet};

use crate::error::AppError;

/// 空間IDに対して値を割り当てる（強制上書き）
pub fn data_insert(
    _layer_id: u64,
    ids: SpatialIdSet,
    _value: &[u8],
) -> Result<(), AppError> {
    for ele in ids.into_single_ids() {
        println!("{},", ele)
    }
    println!("Data Insert Request");
    Ok(())
}

pub fn data_remove(_layer_id: u64, _ids: SpatialIdSet) -> Result<(), AppError> {
    Ok(())
}

/// 空間IDに対して値を割り当てる（値がないIDにのみ書き込む）
/// TODO: 実際のDB操作を実装する
pub fn data_upsert(_layer_id: u64, _ids: SpatialIdSet, _value: &[u8]) -> Result<(), AppError> {
    Ok(())
}
