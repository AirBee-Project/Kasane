use serde::{Deserialize, Serialize};

use crate::r#type::point::{ecef::ECEF, geodetic::Geodetic};

pub mod ecef;
pub mod geodetic;

pub enum PointKind {
    ECEF,
    Geodetic,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Point {
    ECEF(ECEF),
    Geodetic(Geodetic),
}
