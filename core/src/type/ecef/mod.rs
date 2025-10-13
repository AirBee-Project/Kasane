pub mod ecef_to_id;
pub mod ecef_to_point;

#[derive(Debug, Clone, Copy)]
pub struct ECEF {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
