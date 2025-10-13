pub mod center;
pub mod coordinates;
pub mod pure;
pub mod relation;
pub mod scale;
pub mod top;
pub mod value;
pub mod vertex;
pub mod with;
pub mod z_range;

/// Represents a value for a single dimension (F, X, Y, or T) in a SpaceTimeId.
///
/// This enum can represent a single value, a range, or an Any value,
/// corresponding to the extended notation of the spatial ID.
#[derive(
    Debug,
    Clone,
    Copy,
    Hash,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum DimensionRange<T> {
    /// A closed range with a start and end value (e.g., 5:10).
    LimitRange(T, T),
    /// An unlimited range up to an end value (e.g., -:10).
    BeforeUnLimitRange(T),
    /// An unlimited range from a start value onwards (e.g., 5:-).
    AfterUnLimitRange(T),
    /// A single, discrete value.
    Single(T),
    /// An Any value, indicating it applies to all possible values in this dimension (e.g., -).
    Any,
}

use crate::r#type::spacetimeid::z_range::{F_MAX, F_MIN, XY_MAX};
use DimensionRange::{AfterUnLimitRange, Any, BeforeUnLimitRange, LimitRange, Single};
use std::fmt;

impl<T> fmt::Display for DimensionRange<T>
where
    T: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LimitRange(start, end) => write!(f, "{}:{}", start, end),
            BeforeUnLimitRange(v) => write!(f, "-:{}", v),
            AfterUnLimitRange(v) => write!(f, "{}:-", v),
            Single(v) => write!(f, "{}", v),
            Any => write!(f, "-"),
        }
    }
}
#[derive(PartialEq, Debug, Clone, Copy)]
/// * z: The zoom level, a u16 value. It defines the coordinate space boundaries.
/// * f: The value for the F (vertical) dimension as a `DimensionRange<i32>`.
/// * x: The value for the X dimension as a `DimensionRange<u32>`.
/// * y: The value for the Y dimension as a `DimensionRange<u32>`.
/// * i: The time interval in seconds, a `u32` value.
/// * t: The time index value as a `DimensionRange<u32>`.
///
/// # Normalization Rules
///
/// The input `DimensionRange` values are normalized during validation inside `SpaceTimeId::new`
/// according to the following rules:
///
/// ## `Single`
/// - No normalization applied (used as-is).
///
/// ## `LimitRange(start, end)`
/// - If `start == end`: converted to `Single(start)`.
/// - If the range spans the entire valid domain:
///   - For x/y: `0..=2^z - 1`
///   - For f: `-2^z..=2^z - 1`
///   → converted to `Any`.
/// - If `start == 0`: converted to `BeforeUnLimitRange(end)`.
/// - If `end == max`: converted to `AfterUnLimitRange(start)`.
///
/// ## `AfterUnLimitRange(start)`
/// - If `start == 0` (for x/y) or `start == min_f` (for f): converted to `Any`.
/// - If `start == max` (for x/y only): converted to `Single(max)`.
///
/// ## `BeforeUnLimitRange(end)`
/// - If `end == max` (for x/y) or `end == max_f` (for f): converted to `Any`.
/// - If `end == 0` (for x/y) or `end == min_f` (for f): converted to `Single(end)`.
///
/// ## `Any`
/// - No normalization applied (used as-is).
///
/// ## Special case for time dimension `t`
/// - If `i == 0`, then `t` **must** be `Any`. Otherwise, an error is returned.

#[derive(Hash, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SpaceTimeId {
    z: u8,
    f: DimensionRange<i32>,
    x: DimensionRange<u32>,
    y: DimensionRange<u32>,
    i: u32,
    t: DimensionRange<u32>,
}

impl fmt::Display for SpaceTimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}/{}_{}/{}",
            self.z, self.f, self.x, self.y, self.i, self.t
        )
    }
}

impl SpaceTimeId {
    /// Creates a new `SpaceTimeId` and normalizes all dimension ranges.
    ///
    /// # Normalization Rules
    ///
    /// Each dimension (`x`, `y`, `f`, `t`) is normalized so that
    /// redundant or equivalent representations are reduced to a canonical form.
    ///
    /// ## XY dimensions
    /// - `AfterUnLimitRange(0)` → `Any`
    /// - `AfterUnLimitRange(max)` → `Single(max)`
    /// - `BeforeUnLimitRange(max)` → `Any`
    /// - `BeforeUnLimitRange(0)` → `Single(0)`
    ///
    /// ## F dimension
    /// - `AfterUnLimitRange(min_f)` → `Any`
    /// - `AfterUnLimitRange(max_f)` → `Single(max_f)`
    /// - `BeforeUnLimitRange(max_f)` → `Any`
    /// - `BeforeUnLimitRange(min_f)` → `Single(min_f)`
    ///
    /// (`min_f = -2^z`, `max_f = 2^z - 1`)
    ///
    /// ## T dimension
    /// - If `i == 0` (purely spatial ID), then `t` must be `Any`  
    ///   otherwise an error is returned.
    ///
    /// # Errors
    /// - If any range value is outside its valid bounds for the given zoom `z`
    /// - If `t` is not `Any` when `i == 0`
    pub fn new(
        z: u8,
        f: DimensionRange<i32>,
        x: DimensionRange<u32>,
        y: DimensionRange<u32>,
        i: u32,
        t: DimensionRange<u32>,
    ) -> Result<Self, String> {
        if z >= 32 {
            return Err(format!("Zoom level z must be 0..=31. Got {}", z));
        }

        let xy_max = XY_MAX[z as usize];
        let f_min = F_MIN[z as usize];
        let f_max = F_MAX[z as usize];

        fn normalize_xy(
            dim: &DimensionRange<u32>,
            xy_max: u32,
        ) -> Result<DimensionRange<u32>, String> {
            use DimensionRange::*;
            match *dim {
                Single(v) => {
                    if v > xy_max {
                        return Err(format!("XY value {} > max {}", v, xy_max));
                    }
                    Ok(Single(v))
                }
                LimitRange(s, e) => {
                    let (s, e) = if s > e { (e, s) } else { (s, e) };
                    if e > xy_max {
                        return Err(format!("XY range end {} > max {}", e, xy_max));
                    }
                    if s == e {
                        return Ok(Single(s));
                    }
                    if s == 0 && e == xy_max {
                        return Ok(Any);
                    }
                    if s == 0 {
                        return Ok(BeforeUnLimitRange(e));
                    }
                    if e == xy_max {
                        return Ok(AfterUnLimitRange(s));
                    }
                    Ok(LimitRange(s, e))
                }
                AfterUnLimitRange(start) => {
                    if start > xy_max {
                        return Err(format!("XY start {} > max {}", start, xy_max));
                    }
                    if start == 0 {
                        Ok(Any)
                    } else if start == xy_max {
                        Ok(Single(xy_max))
                    } else {
                        Ok(AfterUnLimitRange(start))
                    }
                }
                BeforeUnLimitRange(end) => {
                    if end > xy_max {
                        return Err(format!("XY end {} > max {}", end, xy_max));
                    }
                    if end == xy_max {
                        Ok(Any)
                    } else if end == 0 {
                        Ok(Single(0))
                    } else {
                        Ok(BeforeUnLimitRange(end))
                    }
                }
                Any => Ok(Any),
            }
        }

        fn normalize_f(
            dim: &DimensionRange<i32>,
            f_min: i32,
            f_max: i32,
        ) -> Result<DimensionRange<i32>, String> {
            use DimensionRange::*;
            match *dim {
                Single(v) => {
                    if v < f_min || v > f_max {
                        return Err(format!(
                            "F value {} out of bounds [{}..{}]",
                            v, f_min, f_max
                        ));
                    }
                    Ok(Single(v))
                }
                LimitRange(s, e) => {
                    let (s, e) = if s > e { (e, s) } else { (s, e) };
                    if s < f_min || e > f_max {
                        return Err(format!(
                            "F range {}..{} out of bounds [{}..{}]",
                            s, e, f_min, f_max
                        ));
                    }
                    if s == e {
                        return Ok(Single(s));
                    }
                    if s == f_min && e == f_max {
                        return Ok(Any);
                    }
                    if s == f_min {
                        return Ok(BeforeUnLimitRange(e));
                    }
                    if e == f_max {
                        return Ok(AfterUnLimitRange(s));
                    }
                    Ok(LimitRange(s, e))
                }
                AfterUnLimitRange(start) => {
                    if start < f_min || start > f_max {
                        return Err(format!(
                            "F start {} out of bounds [{}..{}]",
                            start, f_min, f_max
                        ));
                    }
                    if start == f_min {
                        Ok(Any)
                    } else if start == f_max {
                        Ok(Single(f_max))
                    } else {
                        Ok(AfterUnLimitRange(start))
                    }
                }
                BeforeUnLimitRange(end) => {
                    if end < f_min || end > f_max {
                        return Err(format!(
                            "F end {} out of bounds [{}..{}]",
                            end, f_min, f_max
                        ));
                    }
                    if end == f_max {
                        Ok(Any)
                    } else if end == f_min {
                        Ok(Single(f_min))
                    } else {
                        Ok(BeforeUnLimitRange(end))
                    }
                }
                Any => Ok(Any),
            }
        }

        fn normalize_t(dim: &DimensionRange<u32>, i: u32) -> Result<DimensionRange<u32>, String> {
            use DimensionRange::*;
            if i == 0 {
                if *dim != Any {
                    return Err("t must be Any when i = 0".to_string());
                }
                return Ok(Any);
            }
            match *dim {
                Single(_) => Ok(*dim),
                LimitRange(s, e) => {
                    let (s, e) = if s > e { (e, s) } else { (s, e) };
                    if s == e {
                        Ok(Single(s))
                    } else if s == 0 {
                        Ok(BeforeUnLimitRange(e))
                    } else {
                        Ok(LimitRange(s, e))
                    }
                }
                AfterUnLimitRange(start) => {
                    if start == 0 {
                        Ok(Any)
                    } else {
                        Ok(AfterUnLimitRange(start))
                    }
                }
                BeforeUnLimitRange(end) => {
                    if end == 0 {
                        Ok(Single(0))
                    } else {
                        Ok(BeforeUnLimitRange(end))
                    }
                }
                Any => Ok(Any),
            }
        }

        Ok(Self {
            z,
            x: normalize_xy(&x, xy_max)?,
            y: normalize_xy(&y, xy_max)?,
            f: normalize_f(&f, f_min, f_max)?,
            i,
            t: normalize_t(&t, i)?,
        })
    }
}
