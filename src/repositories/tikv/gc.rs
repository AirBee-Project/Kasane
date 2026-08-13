//! 論理削除されたテーブルの実体（シャード・件数キー・値インデックス）を回収する。
//!
//! テーブル削除を 2 段に分けるのは 2 つの理由から：
//!
//! - **テーブル全体の排他が要らなくなる。** データ書き込みはリーフ単位でしかロックを取らない
//!   ので、削除時に実体を消そうとすると並行中の書き込みと衝突しないことを保証できない。
//!   カタログから辿れなくすれば取り残されたキーは誰にも見えない。
//! - **1 トランザクションのサイズ上限に縛られない。** ここは分割コミットで構わない。
//!
//! 掃除は「見つけたものを消す」だけなので冪等。回収待ちの項目を消すのは**その周回で何も
//! 見つからなかったとき**に限る――こうすると、カタログ削除と前後して飛び込んだ書き込みが
//! 残骸を作っても次の周回が拾って収束する。

use std::time::Duration;

use crate::error::AppError;
use crate::models::id::TableId;
use crate::repositories::Storage;

use super::keys;
use super::kv::{self, Reader};
use super::{TikvDb, TikvRead, TikvWrite};

/// TiKV の 1 トランザクションあたりの上限に余裕をもって収まり、かつ往復が
/// 増えすぎない程度の大きさ。
const SWEEP_CHUNK: u32 = 1024;

/// 巨大なテーブルの回収で 1 周が延々と続かないようにする。残りは次の周回で消える。
const MAX_CHUNKS_PER_TABLE: usize = 16;

const DEFAULT_SWEEP_INTERVAL_SECS: u64 = 60;

/// **回収は利用者の書き込みと同じ経路を通る。**
///
/// 息継ぎなしで連投すると悲観ロックとリージョンの資源をこちらが握り続け、投入側が TTL を
/// 超えて失敗する。データベースを 1 つ消すだけで数万〜数十万キーの回収が走るので、その間
/// ずっと投入が詰まる。回収は遅れてよいので、こちらを待たせる。
const CHUNK_DELAY: Duration = Duration::from_millis(200);

/// **実行中の読み取りより長くすること。**
///
/// クエリは開始時に固定した断面を読み続けるので、safepoint がそれを追い越すと読もうとして
/// いたバージョンが消えて失敗する。実測で `write` が最大 20 秒台まで伸びるので、桁で余裕を
/// 取ってある（TiDB の既定も 10 分）。
const DEFAULT_MVCC_RETENTION_SECS: u64 = 600;

/// safepoint を進める間隔の下限。`gc` は第 1 段階でキー空間全体を走査するので安くはない。
const MIN_MVCC_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct GcConfig {
    /// MVCC の古いバージョンを残す時間。**ゼロなら MVCC GC を回さない。**
    pub mvcc_retention: Duration,
    /// 回収の間隔。**ゼロなら回収を行わない**（一括投入の間だけ止める用途）。
    pub interval: Duration,
    /// 1 周で 1 テーブルに費やすチャンク数の上限。テスト以外は [`MAX_CHUNKS_PER_TABLE`]。
    max_chunks_per_table: usize,
    /// チャンク間の待ち。テスト以外は [`CHUNK_DELAY`]。
    chunk_delay: Duration,
}

impl GcConfig {
    pub fn from_env() -> Self {
        use super::init::env_parsed;
        Self {
            mvcc_retention: Duration::from_secs(env_parsed(
                "KASANE_TIKV_MVCC_GC_RETENTION_SECS",
                DEFAULT_MVCC_RETENTION_SECS,
            )),
            interval: Duration::from_secs(env_parsed(
                "KASANE_TIKV_GC_INTERVAL_SECS",
                DEFAULT_SWEEP_INTERVAL_SECS,
            )),
            max_chunks_per_table: MAX_CHUNKS_PER_TABLE,
            chunk_delay: CHUNK_DELAY,
        }
    }

    /// safepoint を進める間隔。保持期間より十分短ければよいので、そこから導出する。
    fn mvcc_interval(&self) -> Duration {
        (self.mvcc_retention / 2).max(MIN_MVCC_INTERVAL)
    }

    /// テスト用。待たずに一気に回収し、MVCC GC は回さない。
    pub fn eager() -> Self {
        Self {
            mvcc_retention: Duration::ZERO,
            interval: Duration::from_secs(DEFAULT_SWEEP_INTERVAL_SECS),
            max_chunks_per_table: 64,
            chunk_delay: Duration::ZERO,
        }
    }
}

/// 存在するかどうかだけを見るので中身に意味はない。
const QUEUED: &[u8] = b"1";

impl<R: Reader> TikvRead<'_, R> {
    pub(super) async fn retired_tables(&self) -> Result<Vec<TableId>, AppError> {
        kv::scan_prefix_keys(&self.txn, &keys::Ns::Garbage.prefix())
            .await?
            .iter()
            .map(|key| keys::table_id_from_garbage_key(key))
            .collect()
    }
}

/// 行列の表現（キーと値）を知るのはこのモジュールだけにしておく。
pub(super) fn retire(table_id: TableId) -> (Vec<u8>, Vec<u8>) {
    (keys::garbage(table_id), QUEUED.to_vec())
}

impl TikvWrite<'_> {
    /// 消し切っていれば（`0` を返すなら）回収待ち行列からも外す。
    ///
    /// 判定と削除を同じトランザクションに入れることで、根拠と除去が食い違わない。
    async fn sweep_table_chunk(&mut self, table_id: TableId) -> Result<usize, AppError> {
        for prefix in keys::table_data_prefixes(table_id) {
            // 1 トランザクションを膨らませないよう、1 チャンク消したら戻る。
            let removed = kv::delete_prefix_chunk(&self.txn, &prefix, SWEEP_CHUNK).await?;
            if removed > 0 {
                return Ok(removed);
            }
        }
        kv::delete(&self.txn, keys::garbage(table_id)).await;
        Ok(0)
    }
}

impl TikvDb {
    /// テストから決定的に呼べるよう、定期実行とは別の関数にしてある。
    #[tracing::instrument(skip_all)]
    pub async fn sweep_retired_tables(&self, config: &GcConfig) -> Result<usize, AppError> {
        let retired = self.read(async |r| r.retired_tables().await).await?;
        let mut total = 0usize;

        for table_id in retired {
            for chunk in 0..config.max_chunks_per_table {
                // 連投すると投入側を詰まらせる（`DEFAULT_CHUNK_DELAY_MS` を参照）。
                if chunk > 0 && !config.chunk_delay.is_zero() {
                    tokio::time::sleep(config.chunk_delay).await;
                }

                let removed = self
                    .write(async move |w| w.sweep_table_chunk(table_id).await)
                    .await?;
                total += removed;

                if removed == 0 {
                    // 同じトランザクションで行列からも外れている。
                    tracing::debug!(%table_id, "reclaimed a retired table");
                    break;
                }
                if chunk + 1 == config.max_chunks_per_table {
                    tracing::debug!(%table_id, "retired table is still being reclaimed");
                }
            }
        }

        Ok(total)
    }

    /// `safepoint = 現在時刻 - retention` を PD へ通知し、それより古いバージョンを落として
    /// よいと判断させる（副作用として古い放置ロックも解決される）。
    ///
    /// **自前で回す必要がある。** TiKV はキーを更新するたびに新しいバージョンを追記し、古い
    /// ものは safepoint を基準にした GC でしか消えない。TiDB を伴う構成では TiDB の GC worker
    /// が safepoint を進めるが、**TiKV を直接使う構成では誰も進めない**ので、放っておくと書いた
    /// 全バージョンが永久に残る。1 件の書き込みがリーフ 1 枚（数百 KB になりうる）を丸ごと
    /// 書き直す構造なので、版はすぐ積み上がる。
    async fn advance_gc_safepoint(&self, retention: Duration) -> Result<(), AppError> {
        use tikv_client::TimestampExt;

        let now = self.client.current_timestamp().await?;

        // タイムスタンプは上位ビットがミリ秒。
        let retention_ms = retention.as_millis() as i64;
        if now.physical <= retention_ms {
            // クラスタが立ち上がって間もない。戻す先が無い。
            return Ok(());
        }
        let safepoint = tikv_client::Timestamp {
            physical: now.physical - retention_ms,
            logical: 0,
            ..Default::default()
        };

        // 返り値が false なのは他が先に進めていたというだけなので、失敗として扱わない。
        let applied = self.client.gc(safepoint.clone()).await?;
        tracing::debug!(
            safepoint = safepoint.version(),
            applied,
            "advanced the MVCC gc safepoint"
        );
        Ok(())
    }

    /// [`spawn_sweeper`](Self::spawn_sweeper) とは**別の仕組み**。
    ///
    /// あちらは「消したテーブルの現行データ」を、こちらは「生きているキーの古い版」を消す。
    /// 片方だけでは容量は下がらない。
    pub fn spawn_gc(&self, config: GcConfig) -> Option<tokio::task::JoinHandle<()>> {
        if config.mvcc_retention.is_zero() {
            tracing::warn!(
                "MVCC garbage collection is disabled (KASANE_TIKV_MVCC_GC_RETENTION_SECS=0); \
                 every version of every key will be kept forever"
            );
            return None;
        }

        let interval = config.mvcc_interval();
        let db = self.clone();
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = db.advance_gc_safepoint(config.mvcc_retention).await {
                    // 進められなくても次の周回で取り返せる。落とす理由はない。
                    tracing::warn!("failed to advance the MVCC gc safepoint: {e}");
                }
            }
        }))
    }

    /// **`connect` には含めない。**
    ///
    /// プロセス全体の寿命を持つ常駐処理なので、起動を決めるのはプロセスの持ち主
    /// （`backend::open`）。接続のたびに常駐タスクが増えると、テストのように何度も接続する
    /// 使い方で掃除が多重に走る。
    pub fn spawn_sweeper(&self, config: GcConfig) -> Option<tokio::task::JoinHandle<()>> {
        if config.interval.is_zero() {
            tracing::warn!(
                "retired-table reclamation is disabled (KASANE_TIKV_GC_INTERVAL_SECS=0); \
                 deleted tables will keep occupying space until it is turned back on"
            );
            return None;
        }

        let db = self.clone();
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(config.interval).await;
                match db.sweep_retired_tables(&config).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!("reclaimed {n} key(s) from retired tables"),
                    Err(e) => tracing::warn!("failed to reclaim retired tables: {e}"),
                }
            }
        }))
    }
}
