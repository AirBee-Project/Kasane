use std::ops::{Add, Mul, Sub};

use crate::r#type::spacetimeid::fmt::Debug;
use crate::r#type::spacetimeid::{DimensionRange, SpaceTimeId};
use DimensionRange::{AfterUnLimitRange, Any, BeforeUnLimitRange, LimitRange, Single};

#[derive(Debug, Clone, Copy)]
pub enum Relation {
    /// 完全一致 A == B
    Equal(SpaceTimeId),
    /// A が B に含まれる (A ⊆ B)
    Subset(SpaceTimeId),
    /// B が A に含まれる (B ⊆ A)
    Superset(SpaceTimeId),
    /// 部分的に重なる (交差あり、完全包含でない)
    Overlap(SpaceTimeId),
    /// 全く重ならない
    Disjoint,
}

/// 2つの `SpaceTimeId` の関係を判定する
pub fn relation(a: SpaceTimeId, b: SpaceTimeId) -> Relation {
    // スケールを合わせる
    let target_z = a.z.max(b.z);
    let target_i = if a.i == 0 && b.i == 0 {
        0
    } else if a.i != 0 && b.i == 0 {
        a.i
    } else if a.i == 0 && b.i != 0 {
        b.i
    } else {
        a.i.min(b.i)
    };

    let a_scaled = a.scale(Some(target_z), Some(target_i)).unwrap();
    let b_scaled = b.scale(Some(target_z), Some(target_i)).unwrap();

    // 各次元で交差を求める
    let f = match same_level_dimension_intersection(a_scaled.f, b_scaled.f) {
        Some(v) => v,
        None => return Relation::Disjoint,
    };

    let x = match same_level_dimension_intersection(a_scaled.x, b_scaled.x) {
        Some(v) => v,
        None => return Relation::Disjoint,
    };

    let y = match same_level_dimension_intersection(a_scaled.y, b_scaled.y) {
        Some(v) => v,
        None => return Relation::Disjoint,
    };

    let t = match same_level_dimension_intersection(a_scaled.t, b_scaled.t) {
        Some(v) => v,
        None => return Relation::Disjoint,
    };

    let intersection = SpaceTimeId::new(target_z, f, x, y, target_i, t).unwrap();

    // 完全一致
    if a_scaled == b_scaled {
        return Relation::Equal(a_scaled);
    }
    // A が B に含まれる
    if intersection == a_scaled {
        return Relation::Subset(a_scaled);
    }
    // B が A に含まれる
    if intersection == b_scaled {
        return Relation::Superset(b_scaled);
    }
    // 部分的に重なる
    Relation::Overlap(intersection)
}

/// 同じスケールでの次元交差を求める
fn same_level_dimension_intersection<T>(
    a: DimensionRange<T>,
    b: DimensionRange<T>,
) -> Option<DimensionRange<T>>
where
    T: Copy
        + Add<Output = T>
        + Sub<Output = T>
        + Mul<Output = T>
        + From<u8>
        + PartialEq
        + std::cmp::PartialOrd
        + Ord
        + Debug,
{
    match a {
        Single(a_v) => match b {
            Single(b_v) => {
                if a_v == b_v {
                    Some(a)
                } else {
                    None
                }
            }
            LimitRange(b_v_s, b_v_e) => {
                if b_v_s <= a_v && a_v <= b_v_e {
                    Some(a)
                } else {
                    None
                }
            }
            AfterUnLimitRange(b_v_s) => {
                if b_v_s <= a_v {
                    Some(a)
                } else {
                    None
                }
            }
            BeforeUnLimitRange(b_v_e) => {
                if a_v <= b_v_e {
                    Some(a)
                } else {
                    None
                }
            }
            Any => Some(a),
        },
        LimitRange(a_v_s, a_v_e) => match b {
            LimitRange(b_v_s, b_v_e) => {
                let start = std::cmp::max(a_v_s, b_v_s);
                let end = std::cmp::min(a_v_e, b_v_e);

                if start <= end {
                    Some(LimitRange(start, end))
                } else {
                    None
                }
            }
            AfterUnLimitRange(b_v_s) => {
                if b_v_s <= a_v_e {
                    let start = std::cmp::max(a_v_s, b_v_s);
                    let end = a_v_e;
                    Some(LimitRange(start, end))
                } else {
                    None
                }
            }
            BeforeUnLimitRange(b_v_e) => {
                if b_v_e >= a_v_s {
                    let start = a_v_s;
                    let end = std::cmp::min(a_v_e, b_v_e);
                    if start <= end {
                        Some(LimitRange(start, end))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Any => Some(a),
            _ => same_level_dimension_intersection(b, a),
        },
        AfterUnLimitRange(a_v_s) => match b {
            AfterUnLimitRange(b_v_s) => Some(AfterUnLimitRange(std::cmp::max(a_v_s, b_v_s))),
            BeforeUnLimitRange(b_v_e) => {
                if a_v_s <= b_v_e {
                    Some(LimitRange(a_v_s, b_v_e))
                } else {
                    None
                }
            }
            Any => Some(a),
            _ => same_level_dimension_intersection(b, a),
        },
        BeforeUnLimitRange(a_v_e) => match b {
            BeforeUnLimitRange(b_v_e) => {
                let end = std::cmp::min(a_v_e, b_v_e);
                Some(BeforeUnLimitRange(end))
            }
            Any => Some(a),
            _ => same_level_dimension_intersection(b, a),
        },
        Any => match b {
            Any => Some(a),
            _ => same_level_dimension_intersection(b, a),
        },
    }
}
