use crate::r#type::{ecef::ECEF, point::Point};

pub fn point_to_ecef(point: Point) -> ECEF {
    // WGS-84 定数
    let a: f64 = 6_378_137.0;
    let inv_f: f64 = 298.257_223_563;
    let f = 1.0 / inv_f;
    let b = a * (1.0 - f);
    let e2 = 1.0 - (b * b) / (a * a);

    let lat = point.latitude.to_radians();
    let lon = point.longitude.to_radians();
    let h = point.altitude;

    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let cos_lon = lon.cos();
    let sin_lon = lon.sin();

    let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();

    let x_f64 = (n + h) * cos_lat * cos_lon;
    let y_f64 = (n + h) * cos_lat * sin_lon;
    let z_f64 = (n * (1.0 - e2) + h) * sin_lat;

    ECEF {
        x: x_f64,
        y: y_f64,
        z: z_f64,
    }
}
