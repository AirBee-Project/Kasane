use crate::r#type::point::{ecef::ECEF, geodetic::Geodetic};

pub fn ecef_to_geodetic(ecef: ECEF) -> Geodetic {
    // WGS84 定数
    let a = 6378137.0_f64; // 長半径
    let inv_f = 298.257223563_f64;
    let f = 1.0 / inv_f;
    let b = a * (1.0 - f);
    let e2 = 1.0 - (b * b) / (a * a);

    let x = ecef.x;
    let y = ecef.y;
    let z = ecef.z;

    let lon = y.atan2(x);
    let p = (x * x + y * y).sqrt();

    // 緯度の初期値（Bowring の公式）
    let mut lat = (z / p).atan2(1.0 - f);
    let mut h = 0.0;

    // Newton-Raphson 反復
    for _ in 0..10 {
        let sin_lat = lat.sin();
        let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();
        h = p / lat.cos() - n;
        let new_lat = (z + e2 * n * sin_lat).atan2(p);

        // 収束チェック（1e-12 ≈ 数 mm）
        if (new_lat - lat).abs() < 1e-12 {
            lat = new_lat;
            break;
        }
        lat = new_lat;
    }

    Geodetic {
        latitude: lat.to_degrees(),
        longitude: lon.to_degrees(),
        altitude: h,
    }
}
