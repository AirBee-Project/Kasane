use std::collections::HashSet;

use crate::r#type::{
    ecef::{ECEF, ecef_to_point::ecef_to_point},
    point::{Point, point_to_ecef::point_to_ecef, point_to_id::point_to_id},
    spacetimeid::SpaceTimeId,
};

/// a と b の間の voxel 線分を返す
pub fn line(z: u8, a: Point, b: Point) -> HashSet<SpaceTimeId> {
    let steps = 50_000;

    let mut result = HashSet::new();

    // Point → ECEF
    let ea = point_to_ecef(a);
    let eb = point_to_ecef(b);

    for i in 0..=steps {
        let t = i as f64 / steps as f64;

        // ECEF補間
        let e = ECEF {
            x: ea.x * (1.0 - t) + eb.x * t,
            y: ea.y * (1.0 - t) + eb.y * t,
            z: ea.z * (1.0 - t) + eb.z * t,
        };

        // ECEF → Point
        let p = ecef_to_point(e);

        // Point → Voxel
        let voxel = point_to_id(z, p);

        result.insert(voxel);
    }

    result
}
