use crate::user_error::UserError;

impl UserError {
    /// LMDB のエラーを雑に UserError に変換する関数
    pub fn from_lmdb_error(e: lmdb::Error) -> Self {
        let location = location!();
        UserError::LmdbError {
            message: format!("{:?}", e),
            location,
        }
    }
}
