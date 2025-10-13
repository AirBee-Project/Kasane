use crate::r#type::{point::Point, spacetimeid::SpaceTimeId};

impl SpaceTimeId {
    /// Returns the eight corner vertices of the spatial volume represented by this `SpaceTimeId`.
    ///
    /// The vertices represent each corner of the cuboid volume defined by the minimum and maximum
    /// values of latitude, longitude, and altitude ranges obtained from the `coordinates()` method.
    pub fn vertex(&self) -> [Point; 8] {
        let coordinates = self.coordinates();

        let lat0 = coordinates.latitude.0;
        let lat1 = coordinates.latitude.1;
        let lng0 = coordinates.longitude.0;
        let lng1 = coordinates.longitude.1;
        let alt0 = coordinates.altitude.0;
        let alt1 = coordinates.altitude.1;

        [
            Point {
                latitude: lat0,
                longitude: lng0,
                altitude: alt0,
            },
            Point {
                latitude: lat0,
                longitude: lng0,
                altitude: alt1,
            },
            Point {
                latitude: lat0,
                longitude: lng1,
                altitude: alt0,
            },
            Point {
                latitude: lat0,
                longitude: lng1,
                altitude: alt1,
            },
            Point {
                latitude: lat1,
                longitude: lng0,
                altitude: alt0,
            },
            Point {
                latitude: lat1,
                longitude: lng0,
                altitude: alt1,
            },
            Point {
                latitude: lat1,
                longitude: lng1,
                altitude: alt0,
            },
            Point {
                latitude: lat1,
                longitude: lng1,
                altitude: alt1,
            },
        ]
    }
}
