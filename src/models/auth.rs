use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // username
    pub uid: String, // user UUID (ハイフン区切り文字列)
    pub ver: u64,    // token version
    pub exp: usize,
}
