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
//!
//! ## 挿入と無効化の競合について
//!
//! ネットワークから取れた値を無条件に挿入すると、次の順序で「無効化されたはずの古い値」が
//! 復活しうる。
//!
//! 1. 読み取り X がネットワークへ `region` の取得を投げる（この時点の断面は古い）。
//! 2. その最中に書き込み Y が `region` をコミットし、[`invalidate`](ShardCache::invalidate)
//!    を呼んでキャッシュを消す。
//! 3. 読み取り X の取得が完了し、（Y より前の）古い値をキャッシュへ書き戻してしまう。
//!
//! これを防ぐため、各キーに「直近の無効化の通し番号」を覚えておく（[`current_version`]）。
//! 読み取り側は取得を始める**前**にこの番号を控え（`read_version`）、取得が終わって挿入する
//! **直前**にもう一度読み直す。その間に無効化が起きていれば番号が進んでいるので、挿入を諦める
//! （[`insert_if_fresh`](ShardCache::insert_if_fresh)）。無効化そのものが競合していても、
//! 通し番号は単調増加なので判定は揺らがない。
//!
//! [`current_version`]: ShardCache::current_version

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use kasane_logic::FlexId;
use moka::sync::Cache;

use super::init::env_parsed;
use super::kv::ShardValue;
use crate::models::id::TableId;

const DEFAULT_MAX_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_TTL_MS: u64 = 500;

/// 値本体以外に見込む固定オーバーヘッド(バイト)。キー(`TableId` + `FlexId`)の実体、
/// `Arc` の制御ブロック、moka 内部の管理コストをまとめて概算したもの。
///
/// これを payload 長へ足さないと、子ポインタだけの小さな中間ノードや「未作成領域」の
/// 確認行(payload 0)が実質タダ扱いになり、大量に溜まると `max_bytes` の見積もりが
/// 実際のヒープ使用量から大きく外れる（内部ノードほど頻繁にキャッシュされるので、
/// 特に効いてくる）。
const ENTRY_OVERHEAD_BYTES: u32 = 128;

/// 無効化の通し番号を覚えておく副表の上限件数。値は `u64` だけなので、本体キャッシュとは
/// 独立にバイト数ではなく件数で抑える。
const INVALIDATION_MARKS_CAPACITY: u64 = 1_000_000;

/// 無効化印の最短 TTL。設定された本体の TTL がこれより短くても、印はこれだけ生かす。
///
/// 印が本体より先に消えると、「取得を始めてからここまでの間に無効化されたか」を
/// 判定できなくなり（[`ShardCache::current_version`] が `0` に戻ってしまう）、
/// モジュール doc の競合対策そのものが働かなくなる。取得1回分の往復時間より
/// 十分長い値にしておく。
const MIN_INVALIDATION_MARK_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct ShardCacheConfig {
    /// L2 の総サイズ上限(バイト、値の実データ量 + 固定オーバーヘッドで数える)。`0` で
    /// L2 を無効化する(`BoundedStale` を指定しても実質 `Strict` と同じ挙動になる)。
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

struct Inner {
    /// 未作成領域(`None`)も、確認済みとして値と同じ枠でキャッシュする
    /// (L1 の `NodeCache` と同じ考え方)。
    values: Cache<(TableId, FlexId), Option<ShardValue>>,
    /// キーごとの「直近で無効化された時点の通し番号」。存在しなければ `0`
    /// (まだ一度も無効化されていない)として扱う。
    invalidated_at: Cache<(TableId, FlexId), u64>,
    /// 通し番号の発行元。`invalidate` のたびに進む。
    ///
    /// TiKV の commit_ts ではなく、あえてプロセスローカルな採番にしている。`invalidate` は
    /// 自インスタンスの書き込みがコミットできた後にしか呼ばれない(`mod.rs` の
    /// `write_retrying` を参照)ので、ここで必要なのは「このプロセス内で、どちらの
    /// invalidate が後か」だけであり、クラスタ全体で意味を持つタイムスタンプは要らない。
    /// commit_ts を配線するには `commit_or_release` の戻り値から変更する必要があり、この
    /// キャッシュの外まで変更が及ぶ。
    seq: AtomicU64,
}

pub(crate) struct ShardCache {
    inner: Option<Inner>,
}

impl ShardCache {
    pub(super) fn new(config: &ShardCacheConfig) -> Self {
        if config.max_bytes == 0 || config.ttl.is_zero() {
            return Self { inner: None };
        }

        let values = Cache::builder()
            .max_capacity(config.max_bytes)
            .weigher(|_key: &(TableId, FlexId), value: &Option<ShardValue>| -> u32 {
                let payload = value.as_ref().map_or(0, |v| v.entry().len());
                payload.saturating_add(ENTRY_OVERHEAD_BYTES as usize).min(u32::MAX as usize) as u32
            })
            .time_to_live(config.ttl)
            .build();
        // 印は本体より長生きさせる。本体の TTL を極端に短く設定されても
        // (`MIN_INVALIDATION_MARK_TTL` 未満)、印だけは競合対策が働くだけの寿命を保つ。
        let invalidated_at = Cache::builder()
            .max_capacity(INVALIDATION_MARKS_CAPACITY)
            .time_to_live(config.ttl.max(MIN_INVALIDATION_MARK_TTL))
            .build();
        Self {
            inner: Some(Inner {
                values,
                invalidated_at,
                seq: AtomicU64::new(0),
            }),
        }
    }

    pub(in crate::repositories::tikv) fn get(
        &self,
        table_id: TableId,
        region: FlexId,
    ) -> Option<Option<ShardValue>> {
        self.inner.as_ref()?.values.get(&(table_id, region))
    }

    /// このキーの「直近の無効化」の通し番号。ネットワークへ取得を投げる**前**に呼び、
    /// 戻り値を [`insert_if_fresh`](Self::insert_if_fresh) の `read_version` に渡すこと。
    pub(in crate::repositories::tikv) fn current_version(
        &self,
        table_id: TableId,
        region: FlexId,
    ) -> u64 {
        let Some(inner) = &self.inner else {
            return 0;
        };
        inner.invalidated_at.get(&(table_id, region)).unwrap_or(0)
    }

    /// `read_version` は取得を始める**前**に [`current_version`](Self::current_version) で
    /// 控えた値。取得が終わった今の通し番号と比べて、その間に無効化が起きていなければ挿入する。
    /// 起きていれば(=今の番号が進んでいれば)、古い値でキャッシュを汚さないよう挿入を諦める。
    pub(in crate::repositories::tikv) fn insert_if_fresh(
        &self,
        table_id: TableId,
        region: FlexId,
        read_version: u64,
        value: Option<ShardValue>,
    ) {
        let Some(inner) = &self.inner else {
            return;
        };
        if inner.invalidated_at.get(&(table_id, region)).unwrap_or(0) > read_version {
            // 取得している間に別の書き込みが割り込んだ。この読み取りが持っている値は
            // その書き込みを見ていないかもしれないので、キャッシュへは反映しない。
            return;
        }
        inner.values.insert((table_id, region), value);
    }

    pub(in crate::repositories::tikv) fn invalidate(&self, table_id: TableId, region: FlexId) {
        let Some(inner) = &self.inner else {
            return;
        };
        // 実際の可視性は `invalidated_at` 自身の挿入・取得(moka 側の同期)が担うので、
        // ここは一意かつ単調増加な番号を払い出せれば十分(`Relaxed` で足りる)。
        let seq = inner.seq.fetch_add(1, Ordering::Relaxed) + 1;
        inner.values.invalidate(&(table_id, region));
        inner.invalidated_at.insert((table_id, region), seq);
    }
}
