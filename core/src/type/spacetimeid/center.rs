use crate::r#type::{
    point::{
        Point, PointKind,
        geodetic::{Geodetic, geodetic_to_ecef::geodetic_to_ecef},
    },
    spacetimeid::SpaceTimeId,
};

impl SpaceTimeId {
    pub fn center(&self, point_kind: PointKind) -> Point {
        let coordinates = self.coordinates();

        let geodetic = Geodetic {
            latitude: (coordinates.latitude.0 + coordinates.latitude.1) / 2.0,
            longitude: (coordinates.longitude.0 + coordinates.longitude.1) / 2.0,
            altitude: (coordinates.altitude.0 + coordinates.altitude.1) / 2.0,
        };

        match point_kind {
            PointKind::ECEF => Point::ECEF(geodetic_to_ecef(geodetic)),
            PointKind::Geodetic => Point::Geodetic(geodetic),
        }
    }
}
