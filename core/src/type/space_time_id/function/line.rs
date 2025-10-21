use std::collections::HashSet;

use crate::r#type::{
    point::{
        Point,
        ecef::{ECEF, ecef_to_id::ecef_to_id},
        geodetic::geodetic_to_ecef::geodetic_to_ecef,
    },
    space_time_id_set::SpaceTimeIdSet,
};

/// a と b の間の voxel 線分を返す
pub fn line(z: u8, a: Point, b: Point) -> SpaceTimeIdSet {
    let steps = 50_000;

    let mut result = SpaceTimeIdSet::new();

    // Point → ECEF
    let ea = match a {
        Point::ECEF(ecef) => ecef,
        Point::Geodetic(geodetic) => geodetic_to_ecef(geodetic),
    };
    let eb = match b {
        Point::ECEF(ecef) => ecef,
        Point::Geodetic(geodetic) => geodetic_to_ecef(geodetic),
    };

    for i in 0..=steps {
        let t = i as f64 / steps as f64;

        // ECEF補間
        let e = ECEF {
            x: ea.x * (1.0 - t) + eb.x * t,
            y: ea.y * (1.0 - t) + eb.y * t,
            z: ea.z * (1.0 - t) + eb.z * t,
        };

        // Point → Voxel
        let voxel = ecef_to_id(z, e);

        result.insert(voxel);
    }

    result
}
