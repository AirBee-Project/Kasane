use crate::r#type::{
    ecef::ECEF,
    point::Point,
    spacetimeid::{DimensionRange, SpaceTimeId},
};

/// Point (lat, lon, alt) を SpaceTimeId に変換
pub fn point_to_id(z: u8, point: Point) -> SpaceTimeId {
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

    SpaceTimeId::new(
        z,
        DimensionRange::Single(f_id),
        DimensionRange::Single(x_id),
        DimensionRange::Single(y_id),
        0,                   // i = 0（時間なし）
        DimensionRange::Any, // t = Any
    )
    .unwrap()
}
