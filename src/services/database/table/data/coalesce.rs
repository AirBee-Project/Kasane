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
use std::time::Duration;

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

/// 何も届かないままこれだけ経ったワーカーは自分から降りる（[`Lanes`] を参照）。
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// テーブルごとのワーカーへの入口。
///
/// 鍵の `TableId` はテーブル作成ごとに新しく振られる UUID なので、**生きている
/// テーブル数ではなく、これまでに作られたテーブル数で増える**。放っておくと
/// 作成と削除を繰り返すだけで入口とタスクが際限なく積もる。
///
/// そこでワーカーは暇が続いたら自分の枠を外して降りる。降ろすのも入れ直すのも
/// この `Mutex` の下で行い、送信もロックを持ったまま行うので、
/// 「降りた直後に送って取りこぼす」は起こらない。
type Lanes = Arc<Mutex<HashMap<TableId, mpsc::UnboundedSender<Submission>>>>;

/// ワーカーへ渡す 1 件ぶんの要求。
struct Submission {
    entry: (SpatialIdSet, Vec<u8>),
    /// 値インデックスへ反映する型。テーブル作成時に決まって以後変わらないので、
    /// 同じテーブルへの要求なら必ず同じ値になる。
    index: Option<TableDataType>,
    /// 結果の返り先。要求側が諦めて消えていたら送信は失敗するが、
    /// **バッチ自体は続行する**（既にコミットされる以上、途中で止める意味がない）。
    reply: oneshot::Sender<Result<(), AppError>>,
}

/// テーブルごとの畳み込み口。
#[derive(Clone)]
pub struct WriteCoalescer {
    db: Db,
    lanes: Lanes,
    /// 1 バッチの上限。`KASANE_WRITE_BATCH` があればそちらを使う。
    ///
    /// `1` を指定すると畳み込みが実質無効になり、1 要求 = 1 トランザクションという
    /// 畳み込み導入前とまったく同じ挙動に戻る。効果の計測と、万一の切り戻しに使う。
    max_batch: usize,
}

impl WriteCoalescer {
    pub fn new(db: Db) -> Self {
        let max_batch = std::env::var("KASANE_WRITE_BATCH")
            .ok()
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_MAX_BATCH);

        Self {
            db,
            lanes: Arc::new(Mutex::new(HashMap::new())),
            max_batch,
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
        self.submit(
            table_id,
            Submission {
                entry: (ids, value),
                index,
                reply,
            },
        )?;

        wait.await.map_err(|_| {
            AppError::InternalError("write coalescer dropped the request".to_string())
        })?
    }

    /// 担当ワーカーへ渡す。居なければ起こす。
    ///
    /// **ロックを持ったまま送る。** ワーカーが降りるのも同じロックの下なので、
    /// 入れ違いで要求が宙に浮くことがない。送信が失敗するのは、降りたワーカーの
    /// 入口がまだ残っていた場合だけで、そのときは起こし直して 1 度やり直す。
    fn submit(&self, table_id: TableId, submission: Submission) -> Result<(), AppError> {
        let mut lanes = self.lanes.lock().expect("coalescer lanes are poisoned");
        let mut submission = submission;

        for _ in 0..2 {
            let sender = lanes
                .entry(table_id)
                .or_insert_with(|| {
                    spawn_worker(
                        self.db.clone(),
                        table_id,
                        self.max_batch,
                        Arc::clone(&self.lanes),
                    )
                })
                .clone();

            match sender.send(submission) {
                Ok(()) => return Ok(()),
                Err(returned) => {
                    lanes.remove(&table_id);
                    submission = returned.0;
                }
            }
        }

        Err(AppError::InternalError(
            "write coalescer is not accepting work".to_string(),
        ))
    }
}

/// 1 テーブルぶんのワーカー。届いた要求を溜めずに取れるだけ取り、1 回で書く。
fn spawn_worker(
    db: Db,
    table_id: TableId,
    max_batch: usize,
    lanes: Lanes,
) -> mpsc::UnboundedSender<Submission> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Submission>();

    tokio::spawn(async move {
        loop {
            let first = match tokio::time::timeout(IDLE_TIMEOUT, rx.recv()).await {
                Ok(Some(first)) => first,
                // 入口がすべて消えた。もう届かない。
                Ok(None) => break,
                Err(_) => {
                    // 暇が続いた。自分の枠を外して降りる。`submit` は同じロックを
                    // 持ったまま送るので、ここで空だと分かれば取りこぼしはない。
                    let mut lanes = lanes.lock().expect("coalescer lanes are poisoned");
                    match rx.try_recv() {
                        Ok(first) => {
                            drop(lanes);
                            first
                        }
                        Err(_) => {
                            lanes.remove(&table_id);
                            break;
                        }
                    }
                }
            };

            // **待たない。** 既に並んでいるぶんだけを取る。空いていれば 1 件で流れ、
            // 混んでいれば前のバッチの処理中に溜まったぶんがここでまとまる。
            let mut batch = vec![first];
            while batch.len() < max_batch {
                match rx.try_recv() {
                    Ok(next) => batch.push(next),
                    Err(_) => break,
                }
            }

            // 同じテーブルなら索引の指定は全員同じなので、先頭のものを使う。
            let index = batch[0].index;
            let (entries, replies): (Vec<_>, Vec<_>) =
                batch.into_iter().map(|s| (s.entry, s.reply)).unzip();
            let batched = entries.len();

            // `entries` はここで手放す。`Storage::write` はやり直しのために
            // クロージャごと複製するので、中で複製し直すと二重に写すことになる。
            let result = db
                .write(async move |w| w.data_insert_many(table_id, index, entries).await)
                .await;

            tracing::debug!(%table_id, batched, ok = result.is_ok(), "flushed a write batch");

            // 全員へ同じ結果を返す。受け取り手が消えていても続ける。
            for reply in replies {
                let _ = reply.send(result.clone());
            }
        }
    });

    tx
}
