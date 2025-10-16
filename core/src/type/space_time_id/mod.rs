use core::fmt;
use std::{
    ops::{Add, Mul, Sub},
    u64,
};

use crate::{
    r#type::space_time_id::z_range::{F_MAX, F_MIN, XY_MAX},
    user_error::UserError,
};

pub struct Dimension<T> {
    start: T,
    end: T,
}
pub mod z_range;

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

///Z=60のIntervalSetに変換
pub struct SpaceTimeId {
    f: Dimension<i64>,
    x: Dimension<u64>,
    y: Dimension<u64>,
    t: Dimension<u64>,
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
        if z > 60 {
            return Err(UserError::ZoomLevelOutOfRange {
                zoom_level: z,
                location: location!(),
            });
        }

        // F/X/Y のチェック + Z=60 正規化
        let f_dim = normalize_and_scale60_f::<i64>(z, f)?;
        let x_dim = normalize_and_scale60_xy::<u64>(z, x, "X")?;
        let y_dim = normalize_and_scale60_xy::<u64>(z, y, "Y")?;

        let t_dim;

        if i == 0 {
            t_dim = Dimension {
                start: 0,
                end: u64::MAX,
            };
        } else {
            // t はオプションで逆転補正
            t_dim = match t {
                (None, None) => Dimension {
                    start: 0,
                    end: u64::MAX,
                },
                (Some(s), None) => Dimension {
                    start: u64::from(s) * u64::from(i),
                    end: u64::MAX,
                },
                (None, Some(e)) => Dimension {
                    start: 0,
                    end: u64::from(e + 1) * u64::from(i),
                },
                (Some(s), Some(e)) => {
                    if s <= e {
                        Dimension {
                            start: u64::from(s) * u64::from(i),
                            end: u64::from(e + 1) * u64::from(i),
                        }
                    } else {
                        Dimension {
                            start: u64::from(e) * u64::from(i),
                            end: u64::from(s + 1) * u64::from(i),
                        }
                    }
                }
            };
        }

        Ok(SpaceTimeId {
            f: f_dim,
            x: x_dim,
            y: y_dim,
            t: t_dim,
        })
    }
}

/// Fの範囲チェック + Z=60 に変換
pub fn normalize_and_scale60_f<T>(
    z: u8,
    f: (Option<i64>, Option<i64>),
) -> Result<Dimension<T>, UserError>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + From<i64>,
{
    // 元のZでの範囲チェック
    let min = F_MIN[z as usize];
    let max = F_MAX[z as usize];

    let clamp_or_error = |value: i64| -> Result<i64, UserError> {
        if value < min || value > max {
            Err(UserError::FOutOfRange {
                f: value,
                z,
                location: location!(),
            })
        } else {
            Ok(value)
        }
    };

    // 範囲チェック後に Dimension<i64> を作る
    let dim_i64 = match f {
        (None, None) => Dimension {
            start: min,
            end: max,
        },
        (Some(s), None) => Dimension {
            start: clamp_or_error(s)?,
            end: max,
        },
        (None, Some(e)) => Dimension {
            start: min,
            end: clamp_or_error(e)?,
        },
        (Some(s), Some(e)) => {
            let (mut start, mut end) = if s <= e { (s, e) } else { (e, s) };
            start = clamp_or_error(start)?;
            end = clamp_or_error(end)?;
            Dimension { start, end }
        }
    };

    // Z=60 にスケール変換
    let coef: i64 = 2_i64.pow(60 - z as u32);
    let one: T = T::from(1);
    let k: T = T::from(coef);

    Ok(Dimension {
        start: T::from(dim_i64.start as i64) * k,
        end: (T::from(dim_i64.end as i64) + one) * k - one,
    })
}

/// X/Y の範囲チェック + Z=60 に変換
pub fn normalize_and_scale60_xy<T>(
    z: u8,
    xy: (Option<u64>, Option<u64>),
    axis: &str,
) -> Result<Dimension<T>, UserError>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T> + From<u64>,
{
    let max_val = XY_MAX[z as usize];

    let clamp_or_error = |value: u64| -> Result<u64, UserError> {
        if value > max_val {
            match axis {
                "X" => Err(UserError::XOutOfRange {
                    x: value,
                    z,
                    location: location!(),
                }),
                "Y" => Err(UserError::YOutOfRange {
                    y: value,
                    z,
                    location: location!(),
                }),
                _ => unreachable!(),
            }
        } else {
            Ok(value)
        }
    };

    // 範囲チェック後に Dimension<u64> を作る
    let dim_u64 = match xy {
        (None, None) => Dimension {
            start: 0,
            end: max_val,
        },
        (Some(s), None) => Dimension {
            start: clamp_or_error(s)?,
            end: max_val,
        },
        (None, Some(e)) => Dimension {
            start: 0,
            end: clamp_or_error(e)?,
        },
        (Some(s), Some(e)) => {
            let (mut start, mut end) = if s <= e { (s, e) } else { (e, s) };
            start = clamp_or_error(start)?;
            end = clamp_or_error(end)?;
            Dimension { start, end }
        }
    };

    // Z=60 にスケール変換
    let coef: u64 = 2_u64.pow(60 - z as u32);
    let one: T = T::from(1);
    let k: T = T::from(coef);

    Ok(Dimension {
        start: T::from(dim_u64.start) * k,
        end: (T::from(dim_u64.end) + one) * k - one,
    })
}
