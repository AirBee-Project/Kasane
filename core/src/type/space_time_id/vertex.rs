use crate::r#type::{
    point::{
        Point, PointKind,
        ecef::ECEF,
        geodetic::{Geodetic, geodetic_to_ecef::geodetic_to_ecef},
    },
    spacetimeid::SpaceTimeId,
};

impl SpaceTimeId {
    pub fn vertex(&self, point_kind: PointKind) -> [Point; 8] {
        let coordinates = self.coordinates();

        let lat = [coordinates.latitude.0, coordinates.latitude.1];
        let lng = [coordinates.longitude.0, coordinates.longitude.1];
        let alt = [coordinates.altitude.0, coordinates.altitude.1];

        let mut vertices = Vec::with_capacity(8);

        for &lat_i in &lat {
            for &lng_i in &lng {
                for &alt_i in &alt {
                    match point_kind {
                        PointKind::Geodetic => {
                            vertices.push(Point::Geodetic(Geodetic {
                                latitude: lat_i,
                                longitude: lng_i,
                                altitude: alt_i,
                            }));
                        }
                        PointKind::ECEF => {
                            let ecef = geodetic_to_ecef(Geodetic {
                                latitude: lat_i,
                                longitude: lng_i,
                                altitude: alt_i,
                            });
                            vertices.push(Point::ECEF(ecef));
                        }
                    }
                }
            }
        }

        // Vec<Point> → [Point; 8] に変換
        vertices
            .try_into()
            .expect("Expected exactly 8 vertices for SpaceTimeId volume")
    }
}
