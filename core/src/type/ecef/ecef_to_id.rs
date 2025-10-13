use crate::r#type::{
    ecef::{ECEF, ecef_to_point::ecef_to_point},
    point::point_to_id::point_to_id,
    spacetimeid::SpaceTimeId,
};

pub fn ecef_to_id(z: u8, ecef: ECEF) -> SpaceTimeId {
    point_to_id(z, ecef_to_point(ecef))
}
