use redb::{CommitError, DatabaseError, StorageError, TableError, TransactionError};

#[derive(Debug)]
pub enum Error {
    FieldNotFound {
        field_name: String,
        location: String,
    },
    FieldAlreadyExists {
        field_name: String,
        location: String,
    },

    DataCorruption {
        location: String,
        kind: DataCorruptionKind,
    },

    Database(DatabaseError),
    TransactionError(TransactionError),
    CommitError(CommitError),
    TableError(TableError),
}

///Diskレベルでエラーはでなかったが、アプリケーションレベルで期待した値の読み書きが行われなかった場合のエラー群
#[derive(Debug)]
pub enum DataCorruptionKind {
    ///フィールド型をu8からDecodeしたときに、対応する数値がなかった場合
    InvalidFieldType,

    //Redbにおいて、Meta Table内にあるべきなデータがなかった場合
    MissingMetadata,
    UnexpectedNull,
    VersionMismatch {
        expected: u32,
        actual: u32,
    },
    ChecksumMismatch,
    InconsistentIndex,
}

impl From<DatabaseError> for Error {
    fn from(err: DatabaseError) -> Self {
        Error::Database(err)
    }
}

impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Self {
        Error::TransactionError(err)
    }
}

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

impl From<StorageError> for Error {
    fn from(err: StorageError) -> Self {
        Error::TableError(redb::TableError::Storage(err))
    }
}
