use serde::Serialize;

#[derive(Serialize)]
pub struct SystemInfoResponse {
    /// サーバーのステータス（通常は "ok"）
    pub status: String,
    /// サーバーのバージョン
    pub version: String,
}
