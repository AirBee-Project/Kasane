use std::f64::consts::PI;

use crate::r#type::spacetimeid::DimensionRange::{
    AfterUnLimitRange, Any, BeforeUnLimitRange, LimitRange, Single,
};
use crate::r#type::spacetimeid::{DimensionRange, SpaceTimeId};
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Coordinates {
    pub latitude: (f64, f64),
    pub longitude: (f64, f64),
    pub altitude: (f64, f64),
}

impl SpaceTimeId {
    /// Converts the `SpaceTimeId` into geographic coordinates (latitude, longitude, and altitude).
    ///
    /// This method interprets the spatial and vertical components of the ID.
    ///
    /// # Returns
    /// A [`Coordinates`] struct containing the latitude, longitude, and altitude ranges
    /// as floating-point tuples `(start, end)`, representing the spatial extent.
    pub fn coordinates(&self) -> Coordinates {
        let n = 2_u32.pow(self.z as u32);

        let longitude = Self::map_range_u32(&self.x, n, Self::longitude);
        let latitude = Self::map_range_u32(&self.y, n, Self::latitude);
        let altitude = Self::map_range_i32(&self.f, n, Self::altitude);

        Coordinates {
            latitude,
            longitude,
            altitude,
        }
    }

    fn longitude(x: u32, n: u32) -> f64 {
        360.0 * (x as f64 / n as f64) - 180.0
    }

    fn latitude(y: u32, n: u32) -> f64 {
        let y_f64 = y as f64;
        let n_f64 = n as f64;
        let exponent = (1.0 - 2.0 * y_f64 / n_f64) * PI;
        let lat_rad = 2.0 * (1.0 - 2.0 / (exponent.exp() + 1.0)).atan();
        lat_rad.to_degrees()
    }

    fn altitude(f: i32, n: u32) -> f64 {
        let f64_val = f as f64;
        let n64_val = n as f64;
        33_554_432.0 * (f64_val / n64_val)
    }

    fn map_range_u32<F>(range: &DimensionRange<u32>, n: u32, func: F) -> (f64, f64)
    where
        F: Fn(u32, u32) -> f64,
    {
        let max_val = n;
        match *range {
            Single(v) => (func(v, n), func(v + 1, n)),
            LimitRange(start, end) => (func(start, n), func(end + 1, n)),
            BeforeUnLimitRange(end) => (func(0, n), func(end + 1, n)),
            AfterUnLimitRange(start) => (func(start, n), func(max_val, n)),
            Any => (func(0, n), func(max_val, n)),
        }
    }

    fn map_range_i32<F>(range: &DimensionRange<i32>, n: u32, func: F) -> (f64, f64)
    where
        F: Fn(i32, u32) -> f64,
    {
        let max_val = n as i32;
        match *range {
            Single(v) => (func(v, n), func(v + 1, n)),
            LimitRange(start, end) => (func(start, n), func(end + 1, n)),
            BeforeUnLimitRange(end) => (func(-max_val, n), func(end + 1, n)),
            AfterUnLimitRange(start) => (func(start, n), func(max_val, n)),
            Any => (func(-max_val, n), func(max_val, n)),
        }
    }
}
