use crate::r#type::spacetimeid::{DimensionRange, SpaceTimeId};

impl SpaceTimeId {
    /// Creates a new `SpaceTimeId` with a different zoom level while keeping all other dimensions unchanged.
    ///
    /// This method validates the new zoom level and returns a new instance with the specified `z` value.
    /// All dimension ranges and time parameters remain the same.
    ///
    /// # Arguments
    ///
    /// * `z` - The new zoom level to set
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `SpaceTimeId` with the updated zoom level, or an error if validation fails.
    ///
    /// # Japanese Note
    ///
    /// z を変更して検証
    pub fn with_z(&self, z: u8) -> Result<Self, String> {
        Self::new(z, self.f, self.x, self.y, self.i, self.t)
    }

    /// Creates a new `SpaceTimeId` with a different F dimension range while keeping all other dimensions unchanged.
    ///
    /// This method validates the new F dimension range and returns a new instance with the specified `f` value.
    /// All other dimension ranges, zoom level, and time parameters remain the same.
    ///
    /// # Arguments
    ///
    /// * `f` - The new F dimension range to set
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `SpaceTimeId` with the updated F dimension, or an error if validation fails.
    ///
    /// # Japanese Note
    ///
    /// f を変更して検証
    pub fn with_f(&self, f: DimensionRange<i32>) -> Result<Self, String> {
        Self::new(self.z, f, self.x, self.y, self.i, self.t)
    }

    /// Creates a new `SpaceTimeId` with a different X dimension range while keeping all other dimensions unchanged.
    ///
    /// This method validates the new X dimension range and returns a new instance with the specified `x` value.
    /// All other dimension ranges, zoom level, and time parameters remain the same.
    ///
    /// # Arguments
    ///
    /// * `x` - The new X dimension range to set
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `SpaceTimeId` with the updated X dimension, or an error if validation fails.
    ///
    /// # Japanese Note
    ///
    /// x を変更して検証
    pub fn with_x(&self, x: DimensionRange<u32>) -> Result<Self, String> {
        Self::new(self.z, self.f, x, self.y, self.i, self.t)
    }

    /// Creates a new `SpaceTimeId` with a different Y dimension range while keeping all other dimensions unchanged.
    ///
    /// This method validates the new Y dimension range and returns a new instance with the specified `y` value.
    /// All other dimension ranges, zoom level, and time parameters remain the same.
    ///
    /// # Arguments
    ///
    /// * `y` - The new Y dimension range to set
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `SpaceTimeId` with the updated Y dimension, or an error if validation fails.
    ///
    /// # Japanese Note
    ///
    /// y を変更して検証
    pub fn with_y(&self, y: DimensionRange<u32>) -> Result<Self, String> {
        Self::new(self.z, self.f, self.x, y, self.i, self.t)
    }

    /// Creates a new `SpaceTimeId` with a different time interval while keeping all other parameters unchanged.
    ///
    /// This method validates the new time interval and returns a new instance with the specified `i` value.
    /// All dimension ranges, zoom level, and T dimension remain the same.
    ///
    /// # Arguments
    ///
    /// * `i` - The new time interval (in seconds) to set
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `SpaceTimeId` with the updated time interval, or an error if validation fails.
    ///
    /// # Japanese Note
    ///
    /// i を変更して検証
    pub fn with_i(&self, i: u32) -> Result<Self, String> {
        Self::new(self.z, self.f, self.x, self.y, i, self.t)
    }

    /// Creates a new `SpaceTimeId` with a different T dimension range while keeping all other dimensions unchanged.
    ///
    /// This method validates the new T dimension range and returns a new instance with the specified `t` value.
    /// All other dimension ranges, zoom level, and time interval remain the same.
    ///
    /// # Arguments
    ///
    /// * `t` - The new T dimension range to set
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `SpaceTimeId` with the updated T dimension, or an error if validation fails.
    ///
    /// # Japanese Note
    ///
    /// t を変更して検証
    pub fn with_t(&self, t: DimensionRange<u32>) -> Result<Self, String> {
        Self::new(self.z, self.f, self.x, self.y, self.i, t)
    }
}
