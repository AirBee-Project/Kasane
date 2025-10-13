use std::collections::HashSet;

use crate::r#type::spacetimeid::DimensionRange::{
    self, AfterUnLimitRange, Any, BeforeUnLimitRange, LimitRange, Single,
};
use crate::r#type::spacetimeid::SpaceTimeId;
use crate::r#type::spacetimeid::z_range::{F_MAX, F_MIN, XY_MAX};

impl SpaceTimeId {
    /// Expands all dimension ranges into individual `SpaceTimeId` instances with single values only.
    ///
    /// This method converts extended notation (Range, Before, After, Any) into a collection of
    /// pure space-time IDs where each dimension uses only `Single` variants. This is useful
    /// for processing operations that require discrete, individual space-time cells.
    ///
    /// # Returns
    ///
    /// A `Vec<SpaceTimeId>` containing all individual IDs that represent the same space-time
    /// region as the original ID, but with each dimension expanded to single values.
    ///
    /// # Note
    ///
    /// The T dimension is preserved as-is and not expanded, maintaining the original temporal range.
    ///
    /// # Japanese Note
    ///
    /// 拡張記法 (Range, Before, After, Any) をすべて展開して
    /// 各次元が Single だけの純粋な ID 群を返す
    pub fn to_single(&self) -> HashSet<SpaceTimeId> {
        let z = self.z();
        let i = self.i();

        let max_xy = XY_MAX[z as usize];
        let max_f = F_MAX[z as usize];
        let min_f = F_MIN[z as usize];

        let expand_u32 = |range: &DimensionRange<u32>, max: u32| -> Vec<u32> {
            match range {
                Single(v) => vec![*v],
                LimitRange(s, e) => (*s..=*e).collect(),
                BeforeUnLimitRange(e) => (0..=*e).collect(),
                AfterUnLimitRange(s) => (*s..=max).collect(),
                Any => (0..=max).collect(),
            }
        };

        let expand_i32 = |range: &DimensionRange<i32>, min: i32, max: i32| -> Vec<i32> {
            match range {
                Single(v) => vec![*v],
                LimitRange(s, e) => (*s..=*e).collect(),
                BeforeUnLimitRange(e) => (min..=*e).collect(),
                AfterUnLimitRange(s) => (*s..=max).collect(),
                Any => (min..=max).collect(),
            }
        };

        let x_vals = expand_u32(&self.x(), max_xy);
        let y_vals = expand_u32(&self.y(), max_xy);
        let f_vals = expand_i32(&self.f(), min_f, max_f);

        let mut result = HashSet::new();

        for &x in &x_vals {
            for &y in &y_vals {
                for &f in &f_vals {
                    result.insert(
                        SpaceTimeId::new(
                            z,
                            Single(f),
                            Single(x),
                            Single(y),
                            i,
                            self.t().clone(), // t はそのまま
                        )
                        .unwrap(),
                    );
                }
            }
        }

        result
    }
}
