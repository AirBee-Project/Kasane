use crate::r#type::spacetimeid::{DimensionRange, SpaceTimeId};
use DimensionRange::{AfterUnLimitRange, Any, BeforeUnLimitRange, LimitRange, Single};

impl SpaceTimeId {
    //この関数はIDを上位のレベルのIDに変換する

    pub fn top(&self, z: u8) -> Result<Self, String> {
        if self.z() == z {
            return Ok(self.clone());
        } else if self.z() < z {
            return Err("自分より小さいIDに変換する場合はScale関数を使用してください".to_string());
        };

        let k = self.z - z;

        //変換ロジック
        return Ok(SpaceTimeId {
            z,
            f: Self::change_top_scale_f(self.f, k),
            x: Self::change_top_scale_xy(self.x, k),
            y: Self::change_top_scale_xy(self.y, k),
            i: self.i,
            t: self.t,
        });
    }

    fn change_top_scale_xy(x: DimensionRange<u32>, k: u8) -> DimensionRange<u32> {
        match x {
            Single(s) => DimensionRange::Single(apply_n_times_u32(s, k)),
            LimitRange(s, e) => {
                DimensionRange::LimitRange(apply_n_times_u32(s, k), apply_n_times_u32(e, k))
            }
            BeforeUnLimitRange(e) => DimensionRange::BeforeUnLimitRange(apply_n_times_u32(e, k)),
            AfterUnLimitRange(s) => DimensionRange::AfterUnLimitRange(apply_n_times_u32(s, k)),
            Any => Any,
        }
    }

    fn change_top_scale_f(x: DimensionRange<i32>, k: u8) -> DimensionRange<i32> {
        match x {
            Single(s) => DimensionRange::Single(apply_n_times_i32(s, k)),
            LimitRange(s, e) => {
                DimensionRange::LimitRange(apply_n_times_i32(s, k), apply_n_times_i32(e, k))
            }
            BeforeUnLimitRange(e) => DimensionRange::BeforeUnLimitRange(apply_n_times_i32(e, k)),
            AfterUnLimitRange(s) => DimensionRange::AfterUnLimitRange(apply_n_times_i32(s, k)),
            Any => Any,
        }
    }
}

fn apply_n_times_u32(x: u32, k: u8) -> u32 {
    x / (1u32 << k)
}

fn apply_n_times_i32(x: i32, k: u8) -> i32 {
    x / (1 << k)
}
