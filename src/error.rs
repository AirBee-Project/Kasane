use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Filed Id Overflow")]
    FiledIdOverflow,

    #[error("Filed Already Exists: {filed_name}")]
    FiledAlreadyExists { filed_name: String },

    #[error("Redb Database Error: {0}")]
    RedbDatabaseError(#[from] redb::DatabaseError),

    #[error("Redb Transaction Error: {0}")]
    RedbTransactionError(#[from] redb::TransactionError),

    #[error("Redb Commit Error: {0}")]
    RedbCommitError(#[from] redb::CommitError),

    #[error("Redb Table Error: {0}")]
    RedbTableError(#[from] redb::TableError),

    #[error("Redb Storage Error: {0}")]
    RedbStorageError(#[from] redb::StorageError),

    #[error("Redb Error: {0}")]
    RedbError(#[from] redb::Error),
}
