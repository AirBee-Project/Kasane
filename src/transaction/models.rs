use kasane_logic::{line, triangle, Coordinate, SetOnMemory};

use crate::error::Error;
pub struct Range(SetOnMemory);
impl Range {
    pub fn triangle(
        z: u8,
        point1: Coordinate,
        point2: Coordinate,
        point3: Coordinate,
    ) -> Result<Self, Error> {
        let mut result = SetOnMemory::new();
        for single_id in triangle(z, point1, point2, point3)? {
            unsafe { result.join_insert_unchecked(&single_id) };
        }
        Ok(Range(result))
    }

    pub fn line(z: u8, point1: Coordinate, point2: Coordinate) -> Result<Self, Error> {
        let mut result = SetOnMemory::new();
        for single_id in line(z, point1, point2)? {
            unsafe { result.join_insert_unchecked(&single_id) };
        }
        Ok(Range(result))
    }

    pub fn point(z: u8, point1: Coordinate) -> Result<Self, Error> {
        let mut result = SetOnMemory::new();
        result.insert(&point1.to_single_id(z)?);
        Ok(Range(result))
    }

    pub fn intersection(&self, other: &SetOnMemory) -> SetOnMemory {
        self.0.intersection(other)
    }

    pub fn difference(&self, other: &SetOnMemory) -> SetOnMemory {
        self.0.difference(other)
    }

    pub fn union(&self, other: &SetOnMemory) -> SetOnMemory {
        self.0.union(other)
    }
}
