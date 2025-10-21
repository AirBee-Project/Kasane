use core::fmt;
use std::u64;
pub mod z_range;
use serde::Serialize;
pub mod function;

use crate::{
    r#type::space_time_id::z_range::{F_MAX, F_MIN, XY_MAX},
    user_error::UserError,
};

pub struct Dimension<T> {
    pub start: T,
    pub end: T,
}

impl<T> fmt::Display for Dimension<T>
where
    T: fmt::Display + PartialEq,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.start, &self.end) {
            (s, e) if s == e => write!(f, "{}", s),
            (s, e) => write!(f, "{}:{}", s, e),
        }
    }
}

/// Z=60 の IntervalSet に変換
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpaceTimeId {
    pub z: u8,
    pub f: (i64, i64),
    pub x: (u64, u64),
    pub y: (u64, u64),
    pub t: (u64, u64),
}

impl fmt::Display for SpaceTimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}:{}/{}:{}/{}:{}_1/{}:{}",
            self.z, self.f.0, self.f.1, self.x.0, self.x.1, self.y.0, self.y.1, self.t.0, self.t.1
        )
    }
}

impl SpaceTimeId {
    pub fn new(
        z: u8,
        f: (Option<i64>, Option<i64>),
        x: (Option<u64>, Option<u64>),
        y: (Option<u64>, Option<u64>),
        i: u32,
        t: (Option<u32>, Option<u32>),
    ) -> Result<Self, UserError> {
        // ZoomLevel のチェック
        // 最大63まで対応できるはず
        if z > 30 {
            return Err(UserError::ZoomLevelOutOfRange {
                zoom_level: z,
                location: location!(),
            });
        }

        // 値の範囲を定義
        let xy_max = XY_MAX[z as usize];
        let f_max = F_MAX[z as usize];
        let f_min = F_MIN[z as usize];

        // X, Y, F の範囲を正規化・検証
        let new_x = normalize_x_range(x, xy_max, z)?;
        let new_y = normalize_y_range(y, xy_max, z)?;
        let new_f = normalize_f_range(f, f_min, f_max, z)?;

        // I と T の計算
        let new_t = if i == 0 {
            (0, u64::MAX)
        } else {
            match t {
                (None, None) => (0, u64::MAX),
                (Some(s), None) => ((s as u64) * (i as u64), u64::MAX),
                (None, Some(e)) => (0, (e * (i + 1)) as u64),
                (Some(s), Some(e)) => {
                    if s < e {
                        ((s as u64) * (i as u64), (e * (i + 1)) as u64)
                    } else {
                        ((e as u64) * (i as u64), (s * (i + 1)) as u64)
                    }
                }
            }
        };

        Ok(Self {
            z,
            f: new_f,
            x: new_x,
            y: new_y,
            t: new_t,
        })
    }
}

fn normalize_x_range(
    x: (Option<u64>, Option<u64>),
    xy_max: u64,
    z: u8,
) -> Result<(u64, u64), UserError> {
    let (s, e) = match x {
        (None, None) => (0, xy_max),
        (Some(s), None) => (s, xy_max),
        (None, Some(e)) => (0, e),
        (Some(s), Some(e)) => {
            if s <= e {
                (s, e)
            } else {
                (e, s)
            }
        }
    };

    valid_range_x(s, 0, xy_max, z)?;
    valid_range_x(e, 0, xy_max, z)?;
    Ok((s, e))
}

fn normalize_y_range(
    y: (Option<u64>, Option<u64>),
    xy_max: u64,
    z: u8,
) -> Result<(u64, u64), UserError> {
    let (s, e) = match y {
        (None, None) => (0, xy_max),
        (Some(s), None) => (s, xy_max),
        (None, Some(e)) => (0, e),
        (Some(s), Some(e)) => {
            if s <= e {
                (s, e)
            } else {
                (e, s)
            }
        }
    };

    valid_range_y(s, 0, xy_max, z)?;
    valid_range_y(e, 0, xy_max, z)?;
    Ok((s, e))
}

fn normalize_f_range(
    f: (Option<i64>, Option<i64>),
    f_min: i64,
    f_max: i64,
    z: u8,
) -> Result<(i64, i64), UserError> {
    let (s, e) = match f {
        (None, None) => (f_min, f_max),
        (Some(s), None) => (s, f_max),
        (None, Some(e)) => (f_min, e),
        (Some(s), Some(e)) => {
            if s <= e {
                (s, e)
            } else {
                (e, s)
            }
        }
    };

    valid_range_f(s, f_min, f_max, z)?;
    valid_range_f(e, f_min, f_max, z)?;
    Ok((s, e))
}

fn valid_range_x(num: u64, min: u64, max: u64, z: u8) -> Result<(), UserError> {
    if (min..=max).contains(&num) {
        Ok(())
    } else {
        Err(UserError::XOutOfRange {
            x: num,
            z,
            location: location!(),
        })
    }
}

fn valid_range_y(num: u64, min: u64, max: u64, z: u8) -> Result<(), UserError> {
    if (min..=max).contains(&num) {
        Ok(())
    } else {
        Err(UserError::YOutOfRange {
            y: num,
            z,
            location: location!(),
        })
    }
}

fn valid_range_f(num: i64, min: i64, max: i64, z: u8) -> Result<(), UserError> {
    if (min..=max).contains(&num) {
        Ok(())
    } else {
        Err(UserError::FOutOfRange {
            f: num,
            z,
            location: location!(),
        })
    }
}
