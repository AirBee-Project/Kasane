use crate::r#type::{point::Point, spacetimeid::SpaceTimeId};

impl SpaceTimeId {
    /// Returns the center point of the spatial volume represented by this `SpaceTimeId`.
    ///
    /// The center is the midpoint of the latitude, longitude, and altitude ranges
    /// obtained from the `coordinates()` method.
    pub fn center(&self) -> Point {
        let coordinates = self.coordinates();

        Point {
            latitude: (coordinates.latitude.0 + coordinates.latitude.1) / 2.0,
            longitude: (coordinates.longitude.0 + coordinates.longitude.1) / 2.0,
            altitude: (coordinates.altitude.0 + coordinates.altitude.1) / 2.0,
        }
    }
}
