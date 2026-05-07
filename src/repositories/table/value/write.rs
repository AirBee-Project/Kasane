use kasane_logic::{IntoSingleIds, SpatialIdSet};

use crate::error::AppError;

/// 空間IDに対して値を割り当てる
/// そこに値がある場合は上書きされる
pub fn value_insert(
    _table_id: u64,
    ids: SpatialIdSet,
    _value: &[u8],
) -> Result<(), AppError> {
    for ele in ids.into_single_ids() {
        println!("{},", ele,)
    }
    println!("Value Insert Request");
    Ok(())
}

pub fn value_remove(_table_id: u64, _ids: SpatialIdSet) -> Result<(), AppError> {
    Ok(())
}

/// 空間IDに対して値を割り当てる
/// そこに値がすでに存在する場合は上書きしない（Upsert）
/// TODO: 実際のDB操作を実装する
pub fn value_upsert(_table_id: u64, _ids: SpatialIdSet, _value: &[u8]) -> Result<(), AppError> {
    Ok(())
}

