use serde::{Deserialize, Serialize};

pub mod ecef_to_geodetic;
pub mod ecef_to_id;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ECEF {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
