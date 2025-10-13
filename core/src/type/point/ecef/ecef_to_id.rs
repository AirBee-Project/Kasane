use crate::r#type::{
    point::{
        ecef::{ECEF, ecef_to_geodetic::ecef_to_geodetic},
        geodetic::geodetic_to_id::geodetic_to_id,
    },
    spacetimeid::SpaceTimeId,
};

pub fn ecef_to_id(z: u8, ecef: ECEF) -> SpaceTimeId {
    geodetic_to_id(z, ecef_to_geodetic(ecef))
}
