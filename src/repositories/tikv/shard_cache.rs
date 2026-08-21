//! プロセス寿命のシャードキャッシュ(L2)。**既定では読み取りに使われない。**
//!
//! [`crate::models::database::table::data::ConsistencyLevel::BoundedStale`] を明示した
//! リクエストだけがこれを参照する([`ShardReader`](super::tree::ShardReader) 経由)。
//!
//! 書き込みが触ったシャードキーは commit 直後に正確に無効化される
//! (`mod.rs` の `write_retrying` を参照)ので、**自インスタンスの書き込みは常に反映される**。
//! 他インスタンスが行った書き込みは、この無効化が届かないため `ttl` の間だけ古いまま
//! 見えることがある — これが `BoundedStale` が受け入れるトレードオフの実体。
//!
//! カタログ・ACL の解決はここに一切含まれない。常に生の読み取りで行われるので、
//! `BoundedStale` を選んでも「見えているデータ = 権限がある」の対応関係は崩れない。

use std::time::Duration;

use kasane_logic::FlexId;
use moka::sync::Cache;

use super::init::env_parsed;
use super::kv::ShardValue;
use crate::models::id::TableId;

const DEFAULT_MAX_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_TTL_MS: u64 = 500;

#[derive(Debug, Clone)]
pub struct ShardCacheConfig {
    /// L2 の総サイズ上限(バイト、値の実データ量で数える)。`0` で L2 を無効化する
    /// (`BoundedStale` を指定しても実質 `Strict` と同じ挙動になる)。
    pub max_bytes: u64,
    /// 他インスタンス発の書き込みを許容する最大の遅れ。`0` で L2 を無効化する。
    pub ttl: Duration,
}

impl ShardCacheConfig {
    pub fn from_env() -> Self {
        Self {
            max_bytes: env_parsed("KASANE_TIKV_SHARD_CACHE_MAX_BYTES", DEFAULT_MAX_BYTES),
            ttl: Duration::from_millis(env_parsed("KASANE_TIKV_SHARD_CACHE_TTL_MS", DEFAULT_TTL_MS)),
        }
    }
}

/// 未作成領域(`None`)も、確認済みとして値と同じ枠でキャッシュする(L1 の `NodeCache` と同じ考え方)。
pub(crate) struct ShardCache {
    inner: Option<Cache<(TableId, FlexId), Option<ShardValue>>>,
}

impl ShardCache {
    pub(super) fn new(config: &ShardCacheConfig) -> Self {
        if config.max_bytes == 0 || config.ttl.is_zero() {
            return Self { inner: None };
        }

        let inner = Cache::builder()
            .max_capacity(config.max_bytes)
            .weigher(|_key: &(TableId, FlexId), value: &Option<ShardValue>| -> u32 {
                // 未作成領域の確認だけの行(`None`)も、キー分くらいの重みは持たせておく。
                value
                    .as_ref()
                    .map_or(1, |v| v.entry().len().min(u32::MAX as usize) as u32)
            })
            .time_to_live(config.ttl)
            .build();
        Self { inner: Some(inner) }
    }

    pub(in crate::repositories::tikv) fn get(
        &self,
        table_id: TableId,
        region: FlexId,
    ) -> Option<Option<ShardValue>> {
        self.inner.as_ref()?.get(&(table_id, region))
    }

    pub(in crate::repositories::tikv) fn insert(
        &self,
        table_id: TableId,
        region: FlexId,
        value: Option<ShardValue>,
    ) {
        if let Some(inner) = &self.inner {
            inner.insert((table_id, region), value);
        }
    }

    pub(in crate::repositories::tikv) fn invalidate(&self, table_id: TableId, region: FlexId) {
        if let Some(inner) = &self.inner {
            inner.invalidate(&(table_id, region));
        }
    }
}
