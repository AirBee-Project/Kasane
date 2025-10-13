use crate::r#type::{
    point::{Point, ecef::ecef_to_id::ecef_to_id, geodetic::geodetic_to_id::geodetic_to_id},
    spacetimeid::SpaceTimeId,
};

fn point(z: u8, point: Point) -> SpaceTimeId {
    match point {
        Point::ECEF(ecef) => ecef_to_id(z, ecef),
        Point::Geodetic(geodetic) => geodetic_to_id(z, geodetic),
    }
}
