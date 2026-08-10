//! LMDB(heed) バックエンドを [`storage`](super::storage) の trait 群へ適合させる層。
//!
//! 実処理は各モジュールに既にある同期メソッドが持っており、ここはそれを非同期 API へ
//! 橋渡しするだけ。LMDB はローカルの mmap なので、これらの `async fn` は実際には
//! await を跨がず即座に値を返す。
//!
//! # なぜクロージャ全体を `spawn_blocking` の中で回すのか
//!
//! LMDB は「1 つのトランザクションは単一スレッドからのみ使うこと」を要求する
//! （`RwTxn` が `Send` であっても、POSIX ではライタミューテックスの所有スレッド制約がある）。
//! そこで [`Storage::read`] / [`Storage::write`] は blocking タスクを 1 つ起こし、
//! **その中でトランザクションを開き、クロージャを最後まで回し、閉じる**。
//! トランザクションがスレッドを跨がないことが構造から保証され、`'static` へ延長するための
//! unsafe な自己参照も要らなくなる。
//!
//! クロージャ内の `.await` は `Handle::block_on` で回す。LMDB 側の Future は即座に
//! 完了するので、ブロッキングスレッドを実質的に占有しない。

use kasane_logic::{FlexId, SpatialIdSet};

use crate::db_init::AppDb;
use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, UpdateTableConstraints,
};
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::{PrivilegeRule, PrivilegeTarget, User, UserMetadata};
use crate::repositories::{
    MetaRead, MetaRepository, ReadRepository, Storage, ValueGroups, WriteRepository,
};

/// 読み取りトランザクションとその上でのリポジトリ操作。
pub struct KasaneDbRead<'a> {
    pub read_txn: heed::RoTxn<'a, heed::WithoutTls>,
    pub db: &'a AppDb,
}

impl<'a> KasaneDbRead<'a> {
    pub fn new(read_txn: heed::RoTxn<'a, heed::WithoutTls>, db: &'a AppDb) -> Self {
        Self { read_txn, db }
    }
}

/// 書き込みトランザクションとその上でのリポジトリ操作。
pub struct KasaneDbWrite<'a> {
    pub write_txn: heed::RwTxn<'a>,
    pub db: &'a AppDb,
}

impl<'a> KasaneDbWrite<'a> {
    pub fn new(write_txn: heed::RwTxn<'a>, db: &'a AppDb) -> Self {
        Self { write_txn, db }
    }

    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}

/// heed のエラーをアプリケーションのエラーへ持ち上げる。
///
/// `AppError` はバックエンド非依存に保ちたいので、具体的なエラー型を知っているのは
/// このモジュールだけにする（feature でバックエンドを差し替える際もここごと入れ替わる）。
impl From<heed::Error> for AppError {
    fn from(error: heed::Error) -> Self {
        AppError::StorageError(error.to_string())
    }
}

/// 点参照 6 つを、同期版 [`MetaRead`] へ委譲して実装する。
macro_rules! impl_meta_repository {
    ($target:ty) => {
        impl MetaRepository for $target {
            async fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError> {
                MetaRead::database_id(self, name)
            }

            async fn table_id(
                &self,
                db_id: DatabaseId,
                table_name: &str,
            ) -> Result<Option<TableId>, AppError> {
                MetaRead::table_id(self, db_id, table_name)
            }

            async fn database_name(&self, db_id: DatabaseId) -> Result<Option<String>, AppError> {
                MetaRead::database_name(self, db_id)
            }

            async fn table_name(&self, table_id: TableId) -> Result<Option<String>, AppError> {
                MetaRead::table_name(self, table_id)
            }

            async fn user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError> {
                MetaRead::user_meta(self, username)
            }

            async fn table_names(&self, db_id: DatabaseId) -> Result<Vec<String>, AppError> {
                MetaRead::table_names(self, db_id)
            }
        }
    };
}

impl_meta_repository!(KasaneDbRead<'_>);
impl_meta_repository!(KasaneDbWrite<'_>);

impl ReadRepository for KasaneDbRead<'_> {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        KasaneDbRead::database_info_impl(self, name)
    }

    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        KasaneDbRead::database_list_impl(self)
    }

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        KasaneDbRead::table_info_impl(self, db_name, table_name)
    }

    async fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        KasaneDbRead::table_list_impl(self, db_name)
    }

    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError> {
        KasaneDbRead::table_list_by_id_impl(self, db_id)
    }

    async fn table_count(&self, table_id: TableId) -> Result<u64, AppError> {
        KasaneDbRead::table_count_impl(self, table_id)
    }

    async fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError> {
        KasaneDbRead::data_get_impl(self, table_id, ids, limit)
    }

    async fn data_filter_eq(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        KasaneDbRead::data_filter_eq_impl(self, table_id, data_type, value)
    }

    async fn data_filter_range(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        KasaneDbRead::data_filter_range_impl(self, table_id, data_type, lo, hi)
    }

    async fn get_user(&self, username: &str) -> Result<Option<User>, AppError> {
        KasaneDbRead::get_user_impl(self, username).await
    }

    async fn require_user(&self, username: &str) -> Result<User, AppError> {
        KasaneDbRead::require_user_impl(self, username).await
    }

    async fn get_all_users(&self) -> Result<Vec<User>, AppError> {
        KasaneDbRead::get_all_users_impl(self)
    }
}

impl WriteRepository for KasaneDbWrite<'_> {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        KasaneDbWrite::database_info_impl(self, name)
    }

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        KasaneDbWrite::table_info_impl(self, db_name, table_name)
    }

    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        KasaneDbWrite::database_create_impl(self, name, description)
    }

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        KasaneDbWrite::database_remove_impl(self, name)
    }

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<(), AppError> {
        KasaneDbWrite::database_update_impl(self, name, new_name, description)
    }

    async fn database_copy(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError> {
        KasaneDbWrite::database_copy_impl(self, src_db_name, copy_name)
    }

    async fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
    ) -> Result<Table, AppError> {
        KasaneDbWrite::table_create_impl(
            self,
            db_name,
            table_name,
            data_type,
            max_zoom_level,
            constraints,
            description,
        )
    }

    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
        validate_existing_data: bool,
    ) -> Result<Table, AppError> {
        KasaneDbWrite::table_update_impl(
            self,
            db_name,
            table_name,
            new_name,
            new_constraints,
            description,
            validate_existing_data,
        )
    }

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        KasaneDbWrite::table_remove_impl(self, db_name, table_name)
    }

    async fn table_copy(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError> {
        KasaneDbWrite::table_copy_impl(
            self,
            src_db_name,
            src_table_name,
            copy_db_name,
            copy_table_name,
        )
    }

    async fn data_insert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        KasaneDbWrite::data_insert_impl(self, table_id, data_type, ids, data)
    }

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        KasaneDbWrite::data_upsert_impl(self, table_id, data_type, ids, data)
    }

    async fn data_remove(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        KasaneDbWrite::data_remove_impl(self, table_id, data_type, ids)
    }

    async fn create_user(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<(), AppError> {
        KasaneDbWrite::create_user_impl(self, username, id, password_hash, privileges).await
    }

    async fn set_password(
        &mut self,
        username: &str,
        password_hash: String,
    ) -> Result<(), AppError> {
        KasaneDbWrite::set_password_impl(self, username, password_hash).await
    }

    async fn grant_privilege(
        &mut self,
        username: &str,
        rule: &PrivilegeRule,
    ) -> Result<(), AppError> {
        KasaneDbWrite::grant_privilege_impl(self, username, rule).await
    }

    async fn revoke_privilege(
        &mut self,
        username: &str,
        target: &PrivilegeTarget,
    ) -> Result<(), AppError> {
        KasaneDbWrite::revoke_privilege_impl(self, username, target).await
    }

    async fn delete_user(&mut self, username: &str) -> Result<(), AppError> {
        KasaneDbWrite::delete_user_impl(self, username).await
    }
}

impl Storage for AppDb {
    type Read<'a> = KasaneDbRead<'a>;
    type Write<'a> = KasaneDbWrite<'a>;

    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.clone();
        let handle = tokio::runtime::Handle::current();
        // blocking タスクは呼び出し元のスパンを引き継がないので、明示的に渡す。
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _guard = span.enter();
            // トランザクションはこの blocking スレッド上で開き、ここで閉じる。
            let r = KasaneDbRead::new(db.env.read_txn()?, &db);
            handle.block_on(f(&r))
        })
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
    }

    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        let db = self.clone();
        let handle = tokio::runtime::Handle::current();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _guard = span.enter();
            let mut w = KasaneDbWrite::new(db.env.write_txn()?, &db);
            // LMDB は単一ライタなので競合でやり直しになることはない。1 回で確定する。
            // エラー時は commit せずに w を drop すると RwTxn は自動で abort される。
            let out = handle.block_on(f(&mut w))?;
            w.commit()?;
            Ok(out)
        })
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
    }
}
