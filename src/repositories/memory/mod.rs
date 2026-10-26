//! インメモリのストレージ（組み込み向け）。利用者は root だけで、ACL は持たない。
//!
//! 読み取りは状態全体の [`Arc`] を共有し、書き込みは状態を複製して書き換えてから差し替える。
//! 失敗した書き込みは複製を捨てるだけで元に戻る。各表も [`Arc`] で持つので、状態の複製は
//! 表の数によらず定数で、書き換えた表だけが [`Arc::make_mut`] で複製される。[`SpatialIdMap`]
//! は複製が O(1) なので、`data` を複製してもテーブル数に比例する分しかかからない。

mod catalog;
mod data;
mod query_source;
mod repository;
mod users;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use spatial_id::collection::SpatialIdMap;

pub use query_source::MemSource;

use super::Storage;
use crate::error::AppError;
use crate::models::database::DatabaseMetadata;
use crate::models::database::table::TableMetadata;
use crate::models::id::{DatabaseId, TableId};

#[derive(Clone, Default)]
pub struct State {
    databases: Arc<BTreeMap<String, DatabaseMetadata>>,
    tables: Arc<BTreeMap<(DatabaseId, String), TableMetadata>>,
    data: Arc<HashMap<TableId, SpatialIdMap<Vec<u8>>>>,
    /// ID の採番。Wasm には時刻も乱数も無いので連番にする。
    last_id: u128,
}

#[derive(Clone, Default)]
pub struct Memory {
    state: Arc<RwLock<Arc<State>>>,
    /// 書き込み同士を直列にする。待つ間に `await` を挟むので非同期の Mutex を使う。
    writer: Arc<tokio::sync::Mutex<()>>,
}

pub struct MemRead(Arc<State>);

pub struct MemWrite(State);

impl Memory {
    pub fn new() -> Self {
        Self::default()
    }

    fn current(&self) -> Result<Arc<State>, AppError> {
        Ok(self.state.read().map_err(poisoned)?.clone())
    }
}

fn poisoned<T>(_: std::sync::PoisonError<T>) -> AppError {
    AppError::Internal("in-memory storage lock was poisoned".to_string())
}

impl Storage for Memory {
    type Read<'a> = MemRead;
    type Write<'a> = MemWrite;
    type QuerySnapshot = Arc<State>;

    async fn query_snapshot(&self) -> Result<Self::QuerySnapshot, AppError> {
        self.current()
    }

    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        f(&MemRead(self.current()?)).await
    }

    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        let _writer = self.writer.lock().await;
        let mut txn = MemWrite(State::clone(&*self.current()?));
        let out = f(&mut txn).await?;
        let mut state = self.state.write().map_err(poisoned)?;
        *state = Arc::new(txn.0);
        Ok(out)
    }
}
