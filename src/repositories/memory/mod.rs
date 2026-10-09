//! メモリ上のバックエンド。テーブルの中身は 1 テーブルにつき 1 つの [`SpatialIdMap`] で持つ。
//!
//! 書き込みトランザクションは [`State`] を複製して適用し、成功したら差し替える。
//! [`SpatialIdMap`] は構造共有する永続木なので、複製しても書き換えた経路しかコピーされない。

mod catalog;
mod data;
mod init;
mod query_source;
mod repository;
mod users;

pub use query_source::TableSource;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use kasane_logic::SpatialIdMap;

use crate::error::AppError;
use crate::models::database::DatabaseMetadata;
use crate::models::database::table::TableMetadata;
use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{DataRole, UserRecord};
use crate::repositories::Storage;

/// ストレージの中身すべて。読み取りは確定済みの [`Arc<State>`] を、書き込みはその複製を通して触る。
#[derive(Clone, Default)]
pub struct State {
    databases: BTreeMap<String, DatabaseMetadata>,
    tables: BTreeMap<(DatabaseId, String), TableMetadata>,
    data: HashMap<TableId, SpatialIdMap<Vec<u8>>>,
    users: BTreeMap<String, UserRecord>,
    acl: BTreeMap<(PrincipalId, DataTarget), DataRole>,
}

/// ストレージのハンドル。
///
/// 書き込みは確定までロックを持ち続けるので、同時の書き込み同士は直列になる。
#[derive(Clone)]
pub struct MemoryDb(Arc<tokio::sync::Mutex<Arc<State>>>);

impl Storage for MemoryDb {
    type Read<'a> = Arc<State>;
    type Write<'a> = State;
    type QuerySnapshot = Arc<State>;

    async fn query_snapshot(&self) -> Result<Arc<State>, AppError> {
        Ok(self.0.lock().await.clone())
    }

    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        let snapshot = self.0.lock().await.clone();
        f(&snapshot).await
    }

    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        let mut committed = self.0.lock().await;
        let mut next = State::clone(&committed);
        // 失敗したら `next` ごと捨てるので、途中までの変更は残らない。
        let out = f(&mut next).await?;
        *committed = Arc::new(next);
        Ok(out)
    }
}
