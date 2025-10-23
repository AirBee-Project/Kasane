use crate::r#type::{point::geodetic::Geodetic, space_time_id::SpaceTimeId};

/// Point (lat, lon, alt) を SpaceTimeId に変換
pub fn geodetic_to_id(z: u8, point: Geodetic) -> SpaceTimeId {
    let lat = point.latitude;
    let lon = point.longitude;
    let alt = point.altitude;

    // ---- 高度 h -> f (Python の h_to_f を Rust に移植) ----
    let factor = 2_f64.powi(z as i32 - 25); // 2^(z-25)
    let f_id = (factor * alt).floor() as i32;

    // ---- 経度 lon -> x ----
    let n = 2u32.pow(z as u32) as f64;
    let x_id = ((lon + 180.0) / 360.0 * n).floor() as u32;

    // ---- 緯度 lat -> y (Web Mercator) ----
    let lat_rad = lat.to_radians();
    let y_id = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n)
        .floor() as u32;

    SpaceTimeId {
        z: z,
        f: (f_id as i64, f_id as i64),
        x: (x_id as u64, x_id as u64),
        y: (y_id as u64, y_id as u64),
        i: 0,
        t: (0, u64::MAX),
    }
}
