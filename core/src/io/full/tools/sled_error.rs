use sled::transaction::TransactionError;

use crate::user_error::UserError;

impl From<sled::Error> for UserError {
    fn from(err: sled::Error) -> Self {
        UserError::SledError {
            message: err.to_string(),
            location: format!("{}:{}", file!(), line!()),
        }
    }
}

impl From<TransactionError<UserError>> for UserError {
    fn from(err: TransactionError<UserError>) -> Self {
        match err {
            TransactionError::Abort(e) => e,
            TransactionError::Storage(e) => UserError::SledTransactionError {
                message: e.to_string(),
                location: format!("{}:{}", file!(), line!()),
            },
        }
    }
}
