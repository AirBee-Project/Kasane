use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lock poisoned")]
    LockPoisoned,

    #[error("Serialization error: {0}")]
    Serialization(String),

    // --- Redb Specific Errors ---
    #[cfg(feature = "redb")]
    #[error("Redb error: {0}")]
    Redb(#[from] redb::Error),

    #[cfg(feature = "redb")]
    #[error("Redb database error: {0}")]
    RedbDatabase(#[from] redb::DatabaseError),

    #[cfg(feature = "redb")]
    #[error("Redb table error: {0}")]
    RedbTable(#[from] redb::TableError),

    #[cfg(feature = "redb")]
    #[error("Redb transaction error: {0}")]
    RedbTransaction(#[from] redb::TransactionError),

    #[cfg(feature = "redb")]
    #[error("Redb storage error: {0}")]
    RedbStorage(#[from] redb::StorageError),

    #[cfg(feature = "redb")]
    #[error("Redb commit error: {0}")]
    RedbCommit(#[from] redb::CommitError),

    // --- TiKV Specific Errors ---
    #[cfg(feature = "tikv")]
    #[error("TiKV error: {0}")]
    Tikv(#[from] tikv_client::Error),
}
