use sled::transaction::{
    ConflictableTransactionError, TransactionError, UnabortableTransactionError,
};

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
            // transaction 内で `abort(user_error)` が呼ばれた場合
            TransactionError::Abort(user_err) => user_err,

            // sled の低レベルストレージエラー
            TransactionError::Storage(e) => UserError::SledError {
                message: format!("{e}"),
                location: "sled::transaction".to_string(),
            },
        }
    }
}

impl From<ConflictableTransactionError<UnabortableTransactionError>> for UserError {
    fn from(err: ConflictableTransactionError<UnabortableTransactionError>) -> Self {
        match err {
            // sledが内部的にリトライするはずの競合
            ConflictableTransactionError::Conflict => UserError::UnKnown {
                message: "Unexpected transaction conflict (should have been retried)".to_string(),
                location: "sled::transaction".to_string(),
            },

            // ユーザー定義のAbort（UnabortableTransactionError）
            ConflictableTransactionError::Abort(inner) => match inner {
                UnabortableTransactionError::Conflict => UserError::UnKnown {
                    message: "Unexpected unabortable conflict during transaction".to_string(),
                    location: "sled::transaction".to_string(),
                },
                UnabortableTransactionError::Storage(e) => UserError::SledError {
                    message: format!("transaction storage error: {e}"),
                    location: "sled::transaction".to_string(),
                },
            },

            // sledの内部ストレージエラー
            ConflictableTransactionError::Storage(e) => UserError::SledError {
                message: format!("sled storage error: {e}"),
                location: "sled::transaction".to_string(),
            },
        }
    }
}
