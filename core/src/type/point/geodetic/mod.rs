pub mod geodetic_to_ecef;
pub mod geodetic_to_id;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Geodetic {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}
