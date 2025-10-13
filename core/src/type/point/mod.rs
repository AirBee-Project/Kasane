pub mod point_to_ecef;
pub mod point_to_id;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}
