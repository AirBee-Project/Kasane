use std::f32::consts::E;

use redb::TableError;
#[cfg(feature = "on_disk")]
use redb::{CommitError, DatabaseError, TransactionError};

pub enum Error {
    KeyNotFound {
        key_name: String,
        location: String,
    },
    KeyAlreadyExists {
        key_name: String,
        location: String,
    },

    #[cfg(feature = "on_disk")]
    Database(DatabaseError),
    #[cfg(feature = "on_disk")]
    TransactionError(TransactionError),
    #[cfg(feature = "on_disk")]
    CommitError(CommitError),
    #[cfg(feature = "on_disk")]
    TableError(TableError),
}

#[cfg(feature = "on_disk")]
impl From<DatabaseError> for Error {
    fn from(err: DatabaseError) -> Self {
        Error::Database(err)
    }
}

#[cfg(feature = "on_disk")]
impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Self {
        Error::TransactionError(err)
    }
}

#[cfg(feature = "on_disk")]
impl From<CommitError> for Error {
    fn from(err: CommitError) -> Self {
        Error::CommitError(err)
    }
}

impl From<TableError> for Error {
    fn from(err: TableError) -> Self {
        Error::TableError(err)
    }
}
