//! バックエンド非依存の抽象 API。

pub mod catalog;
pub mod read;
pub mod storage;
pub mod write;

pub use catalog::CatalogRepository;
pub use read::{ReadRepository, ResolvedTable};
pub use storage::Storage;
pub use write::WriteRepository;

use kasane_logic::FlexId;

/// `data_get` の戻り値：`(値バイト, その値を持つ FlexId 群)` の一覧。
pub type ValueGroups = Vec<(Vec<u8>, Vec<FlexId>)>;

/// 格納バイト列を、クエリで扱う値型へ復元する関数。
pub type DecodeFn<V> = std::sync::Arc<dyn Fn(&[u8]) -> Option<V> + Send + Sync>;
