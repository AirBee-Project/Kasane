//! 同じテーブルへの同時書き込みを 1 つのトランザクションへ畳む（group commit）。
//!
//! # なぜ要るのか
//!
//! このツリーは **1 件の変更でもリーフを丸ごと書き直す**。同じリーフへ N 件を別々の
//! トランザクションで書くと、
//!
//! - リーフのサイズ × N バイトを書く（実測で 513 倍まで膨らむ）
//! - N 個のトランザクションが**同じリーフのロックを奪い合う**
//! - 認証・カタログ解決・prewrite・コミットの固定費を N 回払う
//!
//! 1 つのトランザクションへ畳めば、リーフの書き直しは 1 回、競合はゼロ、固定費も 1 回になる。
//!
//! # 人為的な待ち時間を入れない
//!
//! 「N ミリ秒溜めてから流す」方式は、空いているときに常にその N ミリ秒を損する。
//! そうではなく **前のバッチが処理されている間に届いたものを次のバッチにする**。
//!
//! - 空いているとき: 到着した 1 件をそのまま流す。**追加の遅延はゼロ**
//! - 混んでいるとき: コミットに時間がかかるほど後続が溜まり、自然に大きなバッチになる
//!
//! 負荷が高いほど強く効くので、閾値を人手で調整する必要がない。
//!
//! # 失敗の扱い
//!
//! バッチは 1 つのトランザクションなので、結果は全員で共有する（成功か、同じエラーか）。
//! 値の解釈やズーム解決といった**利用者入力の検証は投入前に終わっている**ので、ここで
//! 起きる失敗はストレージ由来＝どのみち全員が等しく影響を受けるものに限られる。
//!
//! 順序は「同じ空間 ID を複数の要求が書いた場合、後にバッチへ入ったほうが勝つ」。
//! 同時に届いた書き込みの順序はもともと未定義なので、意味論は変わらない。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kasane_logic::SpatialIdSet;
use tokio::sync::{mpsc, oneshot};

use crate::backend::Db;
use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::{Storage, WriteRepository};

/// 1 つのバッチへ載せる要求数の上限。
///
/// 1 トランザクションが触るキー数には TiKV 側の上限（既定 30 万キー / 100MB）があり、
/// 超えると**やり直しても直らない失敗**になる。畳みすぎてそこへ当たると、バッチ全員が
/// 巻き添えで落ちるうえ、再送でまた同じ大きさに畳まれて抜け出せない。
///
/// 上限に対して十分小さく、かつ競合を潰すには十分大きい値にしてある。
const DEFAULT_MAX_BATCH: usize = 256;

/// 1 バッチの上限。`KASANE_WRITE_BATCH` があればそちらを使う。
///
/// `1` を指定すると畳み込みが実質無効になり、1 要求 = 1 トランザクションという
/// 畳み込み導入前とまったく同じ挙動に戻る。効果の計測と、万一の切り戻しに使う。
fn max_batch() -> usize {
    static MAX_BATCH: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MAX_BATCH.get_or_init(|| {
        std::env::var("KASANE_WRITE_BATCH")
            .ok()
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_MAX_BATCH)
    })
}

/// ワーカーへ渡す 1 件ぶんの要求。
struct Submission {
    entry: (SpatialIdSet, Vec<u8>),
    /// 結果の返り先。要求側が諦めて消えていたら送信は失敗するが、
    /// **バッチ自体は続行する**（既にコミットされる以上、途中で止める意味がない）。
    reply: oneshot::Sender<Result<(), AppError>>,
}

/// テーブルごとの畳み込み口。
#[derive(Clone)]
pub struct WriteCoalescer {
    db: Db,
    /// テーブルごとのワーカーへの入口。
    ///
    /// ワーカーは最初の書き込みで起き、以後プロセスの寿命だけ生きる。テーブル数は
    /// 高々カタログの規模なので、増え続けることはない。
    lanes: Arc<Mutex<HashMap<TableId, mpsc::UnboundedSender<Submission>>>>,
}

impl WriteCoalescer {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            lanes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 1 件の書き込みを畳み込みへ渡し、コミットの結果を待つ。
    pub async fn insert(
        &self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        value: Vec<u8>,
    ) -> Result<(), AppError> {
        let (reply, wait) = oneshot::channel();
        let submission = Submission {
            entry: (ids, value),
            reply,
        };

        // 送信が失敗するのはワーカーが死んでいるときだけ。作り直して 1 度だけやり直す。
        if let Err(returned) = self.send(table_id, index, submission) {
            let submission = returned;
            self.restart(table_id);
            self.send(table_id, index, submission).map_err(|_| {
                AppError::InternalError("write coalescer is not accepting work".to_string())
            })?;
        }

        wait.await.map_err(|_| {
            AppError::InternalError("write coalescer dropped the request".to_string())
        })?
    }

    /// 既存のワーカーへ渡す。無ければ起こす。失敗したら要求をそのまま返す。
    fn send(
        &self,
        table_id: TableId,
        index: Option<TableDataType>,
        submission: Submission,
    ) -> Result<(), Submission> {
        let sender = {
            let mut lanes = self.lanes.lock().expect("coalescer lanes are poisoned");
            lanes
                .entry(table_id)
                .or_insert_with(|| spawn_worker(self.db.clone(), table_id, index))
                .clone()
        };
        sender.send(submission).map_err(|e| e.0)
    }

    fn restart(&self, table_id: TableId) {
        self.lanes
            .lock()
            .expect("coalescer lanes are poisoned")
            .remove(&table_id);
    }
}

/// 1 テーブルぶんのワーカー。届いた要求を溜めずに取れるだけ取り、1 回で書く。
fn spawn_worker(
    db: Db,
    table_id: TableId,
    index: Option<TableDataType>,
) -> mpsc::UnboundedSender<Submission> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Submission>();

    tokio::spawn(async move {
        while let Some(first) = rx.recv().await {
            // **待たない。** 既に並んでいるぶんだけを取る。空いていれば 1 件で流れ、
            // 混んでいれば前のバッチの処理中に溜まったぶんがここでまとまる。
            let mut batch = vec![first];
            while batch.len() < max_batch() {
                match rx.try_recv() {
                    Ok(next) => batch.push(next),
                    Err(_) => break,
                }
            }

            let entries: Vec<(SpatialIdSet, Vec<u8>)> =
                batch.iter().map(|s| s.entry.clone()).collect();
            let batched = entries.len();

            let result = db
                .write(async move |w| w.data_insert_many(table_id, index, entries.clone()).await)
                .await;

            tracing::debug!(%table_id, batched, ok = result.is_ok(), "flushed a write batch");

            // 全員へ同じ結果を返す。受け取り手が消えていても続ける。
            for submission in batch {
                let _ = submission.reply.send(result.clone());
            }
        }
    });

    tx
}
