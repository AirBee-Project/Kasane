use std::collections::HashSet;

use crate::r#type::{
    point::{
        Point,
        ecef::{ECEF, ecef_to_geodetic::ecef_to_geodetic, ecef_to_id::ecef_to_id},
        geodetic::{geodetic_to_ecef::geodetic_to_ecef, geodetic_to_id::geodetic_to_id},
    },
    space_time_id_set::SpaceTimeIdSet,
};

pub fn triangle(z: u8, a: Point, b: Point, c: Point) -> SpaceTimeIdSet {
    let steps = 1000;
    let mut voxels_set = SpaceTimeIdSet::new();

    // Point → ECEF
    let ea = match a {
        Point::ECEF(ecef) => ecef,
        Point::Geodetic(geodetic) => geodetic_to_ecef(geodetic),
    };
    let eb = match b {
        Point::ECEF(ecef) => ecef,
        Point::Geodetic(geodetic) => geodetic_to_ecef(geodetic),
    };
    let ec = match b {
        Point::ECEF(ecef) => ecef,
        Point::Geodetic(geodetic) => geodetic_to_ecef(geodetic),
    };

    for i in 0..=steps {
        if i == 0 {
            let p = ecef_to_geodetic(ea);
            let voxel = geodetic_to_id(z, p);
            voxels_set.insert(voxel);
        } else {
            let t = i as f64 / steps as f64;

            // 辺 a-b, a-c を補間
            let line1 = ECEF {
                x: ea.x * (1.0 - t) + eb.x * t,
                y: ea.y * (1.0 - t) + eb.y * t,
                z: ea.z * (1.0 - t) + eb.z * t,
            };
            let line2 = ECEF {
                x: ea.x * (1.0 - t) + ec.x * t,
                y: ea.y * (1.0 - t) + ec.y * t,
                z: ea.z * (1.0 - t) + ec.z * t,
            };

            for j in 0..=i {
                println!("{}", i);

                let s = j as f64 / i as f64;

                // line1 と line2 を補間して内部点を得る
                let e = ECEF {
                    x: line1.x * (1.0 - s) + line2.x * s,
                    y: line1.y * (1.0 - s) + line2.y * s,
                    z: line1.z * (1.0 - s) + line2.z * s,
                };

                // ECEF → Point → Voxel
                let voxel = ecef_to_id(z, e);

                voxels_set.insert(voxel);
            }
        }
    }

    voxels_set
}
