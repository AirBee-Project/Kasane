use crate::r#type::spacetimeid::{
    DimensionRange, SpaceTimeId,
    z_range::{F_MAX, F_MIN, XY_MAX},
};

impl SpaceTimeId {
    // F dimension
    pub fn f_start(&self) -> i32 {
        use DimensionRange::*;
        match self.f {
            Single(v) => v,
            LimitRange(s, _) => s,
            BeforeUnLimitRange(_) => F_MIN[self.z() as usize], // 開始は最小値
            AfterUnLimitRange(s) => s,
            Any => F_MIN[self.z() as usize],
        }
    }

    pub fn f_end(&self) -> i32 {
        use DimensionRange::*;
        match self.f {
            Single(v) => v,
            LimitRange(_, e) => e,
            BeforeUnLimitRange(e) => e,
            AfterUnLimitRange(_) => F_MAX[self.z() as usize], // 終了は最大値
            Any => F_MAX[self.z() as usize],
        }
    }

    // X dimension
    pub fn x_start(&self) -> u32 {
        use DimensionRange::*;
        match self.x {
            Single(v) => v,
            LimitRange(s, _) => s,
            BeforeUnLimitRange(_) => 0,
            AfterUnLimitRange(s) => s,
            Any => 0,
        }
    }

    pub fn x_end(&self) -> u32 {
        use DimensionRange::*;
        let xy_max = XY_MAX[self.z() as usize];
        match self.x {
            Single(v) => v,
            LimitRange(_, e) => e,
            BeforeUnLimitRange(e) => e,
            AfterUnLimitRange(_) => xy_max,
            Any => xy_max,
        }
    }

    // Y dimension
    pub fn y_start(&self) -> u32 {
        use DimensionRange::*;
        match self.y {
            Single(v) => v,
            LimitRange(s, _) => s,
            BeforeUnLimitRange(_) => 0,
            AfterUnLimitRange(s) => s,
            Any => 0,
        }
    }

    pub fn y_end(&self) -> u32 {
        use DimensionRange::*;
        let xy_max = XY_MAX[self.z() as usize];
        match self.y {
            Single(v) => v,
            LimitRange(_, e) => e,
            BeforeUnLimitRange(e) => e,
            AfterUnLimitRange(_) => xy_max,
            Any => xy_max,
        }
    }

    // T dimension
    pub fn t_start(&self) -> u32 {
        use DimensionRange::*;
        match self.t {
            Single(v) => v,
            LimitRange(s, _) => s,
            BeforeUnLimitRange(_) => 0,
            AfterUnLimitRange(s) => s,
            Any => 0,
        }
    }

    pub fn t_end(&self) -> u32 {
        use DimensionRange::*;
        match self.t {
            Single(v) => v,
            LimitRange(_, e) => e,
            BeforeUnLimitRange(e) => e,
            AfterUnLimitRange(_) => u32::MAX, // 制限なし
            Any => u32::MAX,
        }
    }
}
