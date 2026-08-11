//! 論理削除されたテーブルの実体を回収する。
//!
//! テーブル削除（`catalog.rs` の `retire_table`）はカタログ項目を消して
//! 回収待ち行列（`0x09 ‖ table_id`）へ積むだけで完了する。実体である
//! シャード・件数キー・値インデックスの削除はここが後から行う。
//!
//! # なぜ分けるのか
//!
//! - **テーブル全体の排他が要らなくなる。** データ書き込みはリーフ単位でしか
//!   ロックを取らないので（`data.rs`）、削除時に実体を消そうとすると並行中の
//!   書き込みと衝突しないことを保証できない。カタログから辿れなくしてしまえば、
//!   取り残されたキーは誰にも見えない。
//! - **1 トランザクションのサイズ上限に縛られない。** 大きなテーブルの全キーを
//!   1 つのトランザクションへ詰めると TiKV の上限に当たる。ここは分割コミットで
//!   構わない（途中で止まっても、次の掃除が続きから消すだけ）。
//!
//! # 冪等性と取り残し
//!
//! 掃除は「見つけたものを消す」だけなので何度実行してもよい。回収待ちの項目を
//! 消すのは**その周回で何も見つからなかったとき**に限る。こうすると、カタログ削除と
//! 前後して飛び込んだ書き込みが残骸を作っても、次の周回が拾って収束する。
//!
//! クラスタは複数の Kasane インスタンスで共有されうるので、掃除が同時に走ることも
//! ある。同じキーを消す操作は重複しても問題なく、競合すれば通常のリトライで解決する。

use std::time::Duration;

use crate::error::AppError;
use crate::models::id::TableId;
use crate::repositories::Storage;

use super::keys;
use super::kv::{self, Reader};
use super::{TikvDb, TikvRead, TikvWrite, to_app_error};

/// 1 トランザクションで消すキー数の上限。
///
/// TiKV の 1 トランザクションあたりの上限に余裕をもって収まり、かつ往復が
/// 増えすぎない程度の大きさ。
const SWEEP_CHUNK: u32 = 1024;

/// 1 回の掃除で 1 テーブルに費やすチャンク数の上限（既定）。
///
/// 巨大なテーブルの回収で 1 周が延々と続かないようにする。残りは次の周回で消える。
const DEFAULT_MAX_CHUNKS_PER_TABLE: usize = 16;

/// 定期掃除の間隔（既定）。
const DEFAULT_SWEEP_INTERVAL_SECS: u64 = 60;

/// チャンクとチャンクの間に空ける時間（既定）。
///
/// **回収は利用者の書き込みと同じ経路を通る。** 1024 キーの削除を息継ぎなしで
/// 連投すると、悲観ロックとリージョンの資源をこちらが握り続け、投入側の
/// トランザクションが TTL を超えて失敗する。データベースを 1 つ消すだけで
/// 数万〜数十万キーの回収が走るので、その間ずっと投入が詰まることになる。
///
/// 回収は**遅れてよい**（消せなかった分は次の周回で消える）。急ぐ理由が無いほうを
/// 待たせるのが正しい。
const DEFAULT_CHUNK_DELAY_MS: u64 = 200;

/// MVCC の古いバージョンを残す時間（既定）。
///
/// **実行中の読み取りより長くすること。** クエリは開始時にタイムスタンプを固定して
/// その断面を読み続ける（`query_snapshot`）。safepoint がその断面を追い越すと、
/// 読もうとしていたバージョンが消えて実行中のクエリが失敗する。
///
/// 書き込みトランザクションも同様で、コミットまでに safepoint を跨ぐと巻き戻される。
/// 実測で `write` が最大 20 秒台まで伸びることがあるので、桁で余裕を取ってある
/// （TiDB の既定も 10 分）。
const DEFAULT_MVCC_RETENTION_SECS: u64 = 600;

/// MVCC GC を回す間隔（既定）。
///
/// `gc` は第 1 段階で**キー空間全体を走査してロックを解決する**ので安くはない。
/// 保持期間より十分短ければよく、頻繁に回す意味はない。
const DEFAULT_MVCC_INTERVAL_SECS: u64 = 300;

/// 回収の進め方。環境変数で調整できる。
#[derive(Debug, Clone)]
pub struct GcConfig {
    /// MVCC GC を回す間隔。**ゼロなら回さない。**
    pub mvcc_interval: Duration,
    /// MVCC の古いバージョンを残す時間。
    pub mvcc_retention: Duration,
    /// 掃除の間隔。**ゼロなら回収を行わない**（一括投入の間だけ止める用途）。
    pub interval: Duration,
    /// チャンクとチャンクの間に空ける時間。
    pub chunk_delay: Duration,
    /// 1 周で 1 テーブルに費やすチャンク数の上限。
    pub max_chunks_per_table: usize,
}

impl GcConfig {
    /// 環境変数から組み立てる。
    ///
    /// - `KASANE_TIKV_MVCC_GC_INTERVAL_SECS`（既定 300、`0` で MVCC GC を止める）
    /// - `KASANE_TIKV_MVCC_GC_RETENTION_SECS`（既定 600）
    /// - `KASANE_TIKV_GC_INTERVAL_SECS`（既定 60、`0` で回収を止める）
    /// - `KASANE_TIKV_GC_CHUNK_DELAY_MS`（既定 200）
    /// - `KASANE_TIKV_GC_MAX_CHUNKS`（既定 16）
    pub fn from_env() -> Self {
        fn num(name: &str, default: u64) -> u64 {
            std::env::var(name)
                .ok()
                .and_then(|raw| raw.trim().parse::<u64>().ok())
                .unwrap_or(default)
        }
        Self {
            mvcc_interval: Duration::from_secs(num(
                "KASANE_TIKV_MVCC_GC_INTERVAL_SECS",
                DEFAULT_MVCC_INTERVAL_SECS,
            )),
            mvcc_retention: Duration::from_secs(num(
                "KASANE_TIKV_MVCC_GC_RETENTION_SECS",
                DEFAULT_MVCC_RETENTION_SECS,
            )),
            interval: Duration::from_secs(num(
                "KASANE_TIKV_GC_INTERVAL_SECS",
                DEFAULT_SWEEP_INTERVAL_SECS,
            )),
            chunk_delay: Duration::from_millis(num(
                "KASANE_TIKV_GC_CHUNK_DELAY_MS",
                DEFAULT_CHUNK_DELAY_MS,
            )),
            max_chunks_per_table: num(
                "KASANE_TIKV_GC_MAX_CHUNKS",
                DEFAULT_MAX_CHUNKS_PER_TABLE as u64,
            ) as usize,
        }
    }

    /// 待たずに一気に回収する設定（テスト用）。MVCC GC は回さない。
    pub fn eager() -> Self {
        Self {
            mvcc_interval: Duration::ZERO,
            mvcc_retention: Duration::from_secs(DEFAULT_MVCC_RETENTION_SECS),
            interval: Duration::from_secs(DEFAULT_SWEEP_INTERVAL_SECS),
            chunk_delay: Duration::ZERO,
            max_chunks_per_table: 64,
        }
    }
}

/// 回収待ち行列へ積むときの値。存在するかどうかだけを見るので中身に意味はない。
const QUEUED: &[u8] = b"1";

impl<R: Reader> TikvRead<'_, R> {
    /// 回収待ちのテーブル一覧。
    pub(super) async fn retired_tables(&self) -> Result<Vec<TableId>, AppError> {
        kv::scan_prefix_keys(&self.txn, &keys::Ns::Garbage.prefix())
            .await?
            .iter()
            .map(|key| keys::table_id_from_garbage_key(key))
            .collect()
    }
}

impl TikvWrite<'_> {
    /// テーブルを回収待ち行列へ積む。
    ///
    /// 行列の表現（キーと値）を知るのはこのモジュールだけにしておきたいので、
    /// カタログ側からは「回収に回す」とだけ言えるようにしてある。
    pub(super) fn retire(&self, table_id: TableId) -> (Vec<u8>, Vec<u8>) {
        (keys::garbage(table_id), QUEUED.to_vec())
    }

    /// 1 テーブル分の実体を 1 チャンクだけ削除し、消した件数を返す。
    ///
    /// 消し切っていれば（`0` を返すなら）そのまま回収待ち行列からも外す。
    /// 判定と削除を同じトランザクションに入れることで、「何も見つからなかった」
    /// 根拠と行列からの除去が食い違わない。
    async fn sweep_table_chunk(&mut self, table_id: TableId) -> Result<usize, AppError> {
        for prefix in keys::table_data_prefixes(table_id) {
            // 1 トランザクションを膨らませないよう、1 チャンク消したら戻る。
            let removed = kv::delete_prefix_chunk(&self.txn, &prefix, SWEEP_CHUNK).await?;
            if removed > 0 {
                return Ok(removed);
            }
        }
        kv::delete(&self.txn, keys::garbage(table_id)).await?;
        Ok(0)
    }
}

impl TikvDb {
    /// 回収待ちのテーブルを 1 周ぶん掃除する。消したキーの総数を返す。
    ///
    /// テストから決定的に呼べるよう、定期実行とは別の関数にしてある。
    #[tracing::instrument(skip_all)]
    pub async fn sweep_retired_tables(&self, config: &GcConfig) -> Result<usize, AppError> {
        let retired = self.read(async |r| r.retired_tables().await).await?;
        let mut total = 0usize;

        for table_id in retired {
            for chunk in 0..config.max_chunks_per_table {
                // 息継ぎを入れる。**回収は利用者の書き込みと同じ経路を通る**ので、
                // 連投すると投入側を詰まらせる（`DEFAULT_CHUNK_DELAY_MS` の注記を参照）。
                if chunk > 0 && !config.chunk_delay.is_zero() {
                    tokio::time::sleep(config.chunk_delay).await;
                }

                let removed = self
                    .write(async move |w| w.sweep_table_chunk(table_id).await)
                    .await?;
                total += removed;

                if removed == 0 {
                    // 消し切ったので、同じトランザクションで行列からも外れている。
                    tracing::debug!(%table_id, "reclaimed a retired table");
                    break;
                }
                if chunk + 1 == config.max_chunks_per_table {
                    // 残りは次の周回に任せる。
                    tracing::debug!(%table_id, "retired table is still being reclaimed");
                }
            }
        }

        Ok(total)
    }

    /// MVCC の古いバージョンを回収できるようにする（safepoint を進める）。
    ///
    /// # なぜ自前で回す必要があるのか
    ///
    /// TiKV はキーを更新するたびに新しいバージョンを**追記**し、古いものは GC でしか
    /// 消えない。そしてその GC は「PD が保持する safepoint」を基準に動く。
    ///
    /// TiDB を伴う構成では TiDB の GC worker が safepoint を進めるが、**Kasane のように
    /// TiKV を直接使う構成では誰も進めない**。放っておくと safepoint はゼロのままで、
    /// **書いた全バージョンが永久に残る**。
    ///
    /// 1 件の書き込みがリーフ 1 枚（数百 KB になりうる）を丸ごと書き直す構造なので、
    /// 同じ領域を更新し続けると版が積み上がる。定常状態のデータ量が論理的な量と
    /// かけ離れていく原因はここにある。
    ///
    /// # 何をするか
    ///
    /// `safepoint = 現在時刻 - retention` を PD へ通知する。TiKV はそれより古い
    /// バージョンを（コンパクションの過程で）落としてよいと判断する。
    /// 副作用として、safepoint より古い放置ロックも解決される。
    ///
    /// **retention は実行中の読み取りより長くすること**（`DEFAULT_MVCC_RETENTION_SECS`）。
    async fn advance_gc_safepoint(&self, retention: Duration) -> Result<(), AppError> {
        use tikv_client::TimestampExt;

        let now = self.client.current_timestamp().await.map_err(to_app_error)?;

        // タイムスタンプは上位ビットがミリ秒。保持期間ぶん手前へ戻した時刻を safepoint にする。
        let retention_ms = retention.as_millis() as i64;
        if now.physical <= retention_ms {
            // クラスタが立ち上がって間もない。戻す先が無いので何もしない。
            return Ok(());
        }
        let safepoint = tikv_client::Timestamp {
            physical: now.physical - retention_ms,
            logical: 0,
            ..Default::default()
        };

        // 返り値は「要求どおりの safepoint になったか」。他が先に進めていれば false になるが、
        // それは競合ではなく単に相手のほうが新しいというだけなので、失敗として扱わない。
        let applied = self
            .client
            .gc(safepoint.clone())
            .await
            .map_err(to_app_error)?;
        tracing::debug!(
            safepoint = safepoint.version(),
            applied,
            "advanced the MVCC gc safepoint"
        );
        Ok(())
    }

    /// MVCC GC を定期的に回す。設定がゼロなら回さない。
    ///
    /// 回収待ちテーブルの掃除（[`spawn_sweeper`](Self::spawn_sweeper)）とは**別の仕組み**。
    /// あちらは「消したテーブルの現行データ」を消す。こちらは「生きているキーの古い版」を
    /// 消せるようにする。片方だけでは容量は下がらない。
    pub fn spawn_gc(&self, config: GcConfig) -> Option<tokio::task::JoinHandle<()>> {
        if config.mvcc_interval.is_zero() {
            tracing::warn!(
                "MVCC garbage collection is disabled (KASANE_TIKV_MVCC_GC_INTERVAL_SECS=0);                  every version of every key will be kept forever"
            );
            return None;
        }

        let db = self.clone();
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(config.mvcc_interval).await;
                if let Err(e) = db.advance_gc_safepoint(config.mvcc_retention).await {
                    // 進められなくても次の周回で取り返せる。落とす理由はない。
                    tracing::warn!("failed to advance the MVCC gc safepoint: {e}");
                }
            }
        }))
    }

    /// 定期掃除を開始し、その [`JoinHandle`](tokio::task::JoinHandle) を返す。
    ///
    /// **接続を開くこと自体には掃除は含まれない。** これはプロセス全体の寿命を持つ
    /// 常駐処理なので、起動を決めるのはプロセスの持ち主（`backend::open`）であって
    /// `connect` ではない。接続のたびに常駐タスクが増えると、テストのように何度も
    /// 接続する使い方で掃除が多重に走る。
    ///
    /// 失敗しても落とさない。回収は遅れても実害がなく、次の周回で再試行される。
    pub fn spawn_sweeper(&self, config: GcConfig) -> Option<tokio::task::JoinHandle<()>> {
        if config.interval.is_zero() {
            tracing::warn!(
                "retired-table reclamation is disabled (KASANE_TIKV_GC_INTERVAL_SECS=0);                  deleted tables will keep occupying space until it is turned back on"
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
