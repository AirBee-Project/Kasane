use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// 利用者名。
    pub sub: String,
    /// 利用者の UUID。同名で作り直した利用者のトークンを区別する。
    pub uid: String,
    /// トークンの世代（[`UserRecord::token_version`](crate::models::users::UserRecord::token_version)）。
    pub ver: u64,
    /// 失効時刻（UNIX 秒）。
    pub exp: usize,
}
