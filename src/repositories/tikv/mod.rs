//! TiKV バックエンド。
//!
//! ファイル構成は LMDB 実装（`super::lmdb`）と対になっている。
//!
//! | ファイル | 役割 |
//! |---|---|
//! | `mod.rs` | ストレージ本体と、トランザクション境界（[`Storage`] 実装） |
//! | `init.rs` | このバックエンド固有の初期化設定 |
//! | `gc.rs` | 論理削除されたテーブルの実体を回収する（このバックエンド固有） |
//! | `keys.rs` | キーのバイト表現 |
//! | `kv.rs` | このバックエンド固有の低レベルアクセス（LMDB 側は `shard.rs`） |
//! | `catalog.rs` | データベース・テーブルのカタログ操作 |
//! | `data.rs` | FlexTree のデータ操作 |
//! | `users.rs` | ユーザーと権限 |
//! | `query_source.rs` | クエリ実行器への入力源 |
//! | `repository.rs` | 抽象 trait への適合 |
//!
//! # 読み書きの噛み合わせ方
//!
//! 排他の付け方は経路によって違う。**片方だけ見て他方を真似すると壊れる**ので、
//! なぜ違うのかをここに書いておく。
//!
//! ## データ経路：`batch_get_for_update`（悲観）
//!
//! TiKV の悲観ロックは取得時に取り直した `for_update_ts` で取られるのに対し、
//! `txn.get()` はトランザクション開始時の `start_ts` スナップショットを読む。
//! そのため「ロックしてから `get` する」と、ロック取得前にコミットされた他者の変更を
//! 見落として lost update になる。シャードの読み書きはキー単位で完結するので、
//! **ロックと最新値の取得を 1 リクエストで行う** `batch_get_for_update` を使う
//! （[`kv::lock_shards`]）。返る値は `for_update_ts` 時点のものなので食い違いが起きない。
//!
//! テーブル全体を排他しないため、**別のリーフへの書き込みは並列に流れる**。
//! 複数の Kasane インスタンスが同じクラスタへ書き込む構成でも同じ。
//! なぜリーフ単位のロックで足りるのかは `data.rs` の書き込みの節を参照。
//!
//! ここを楽観トランザクションへ寄せたことがある。不変条件は楽観でも通用し、往復も
//! 転送量も減るのだが、**競合の濃いワークロードで書き込みが落ちた**ので戻した。
//! 経緯と実測は `data.rs` に残してある。
//!
//! ## カタログ経路：ロック専用トランザクション（**悲観のまま**）
//!
//! データベース配下のテーブル一覧のように、**範囲スキャンから変更を導く**操作は
//! 楽観の競合検査では守れない。検査するのは「自分が書くキー」だけなので、
//! ファントムが素通りする。
//!
//! > A: `database_remove("db")` … 配下を走査して t1, t2 と db を消す
//! > B: `table_create("db", "t3")` … db の存在を確認して t3 を作る
//!
//! 両者が書くキーは重ならないため、楽観では**どちらも成功して t3 が取り残される**。
//! 読んだ範囲に対する排他が要るので、ここは TiKV 側の実ロックを使う。
//!
//! そこで LMDB の `env.write_txn()` と同じ順序――**ライタミューテックス取得 →
//! スナップショット確定**――を 2 つのトランザクションに分けて再現する。
//!
//! 1. ロック専用トランザクション（[`lock_options`]、**悲観**）でロックキーを取得する
//! 2. **その後に**作業トランザクションを開始する（start_ts が前任者のコミットより後になる）
//! 3. 作業をコミットする
//! 4. ロック専用トランザクションを rollback して解放する
//!
//! ロック側は常に rollback で終わるので、ロックキーには MVCC のバージョンが作られない。
//!
//! **[`lock_options`] を [`write_options`] と同じにしてはならない。** 楽観の
//! `lock_keys` はローカルバッファへ印を付けるだけで RPC を出さず、検査はコミット時に
//! 行われる。ロック側は必ず rollback するので、その検査は永久に行われない――
//! つまり**排他が黙って消える**。コンパイルは通り、競合を能動的に起こさないテストも
//! 通ってしまう。壊れ方は「複数インスタンスでだけ整合性が崩れる」形になる。
//!
//! こちらを使うのはカタログとユーザーの操作だけで、いずれも頻度が低い。
//!
//! # 必要なロックをどう知るか
//!
//! `database_remove` のように「ロックを取って初めて対象が判る」操作があるため、
//! 呼び出し前にロック集合を列挙することはできない。書き込みクロージャは競合時の
//! やり直しに備えて元々**複数回呼ばれてよい**設計なので、これを利用する：
//!
//! - 各操作は触る範囲を [`TikvWrite::require_lock`] で宣言する
//! - まだ保持していないロックが宣言されたら、その場で巻き戻して
//!   **集めたロックを揃えた状態でやり直す**
//!
//! `Storage::write` のシグネチャは変わらず、サービス層はロックの存在を知らないままでいられる。
//!
//! ロック集合が空の周回ではロック用トランザクションを開かない（空の `lock_keys` は
//! 何もしないのに、開くだけで PD からタイムスタンプを取ることになる）。作業
//! トランザクションも実際にデータへ触れるまで開かない（[`kv::LazyTxn`]）。
//! そのため**ロックを 1 つも宣言しないデータ経路はトランザクション 1 本**で完結し、
//! カタログ経路もロック用と作業用の 2 本で済む。
//!
//! # デッドロックしない理由
//!
//! 明示ロック（カタログ経路）は、保持中／不足中のどちらも [`BTreeSet<Vec<u8>>`] で
//! 持つので、取得は常に**ロックキーのバイト昇順**になる。キーは `0x7F ‖ scope ‖ id`
//! （`keys.rs`）で、[`LockScope`] の判別値が粗い粒度から順に振られているため、
//! この昇順がそのままロック階層の順序になる。全操作が同じ全順序で取るので、
//! 待ちグラフに循環ができない。
//!
//! データ経路の [`kv::lock_shards`] も、キーを `BTreeMap` に集めてから昇順で要求する
//! （1 リクエストに載りきらない場合の分割でも順序は保たれる）。
//!
//! 順序は集合の型が決めるので、呼び出し側が並べ替える必要はない。ただし
//! [`tikv_client::Transaction::lock_keys`] はリージョンごとにリクエストを束ねるため、
//! 1 回の呼び出しの**内部**での取得順までは保証されない。そこで最後の砦として、
//! TiKV 側が検出したデッドロックは [`is_retryable`] が拾ってやり直す。

mod catalog;
mod data;
mod gc;
mod init;
mod keys;
mod kv;
mod query_source;
mod repository;
mod users;

pub use init::TikvConfig;
pub use query_source::TikvTableSource;

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use rand_core::RngCore;
use tikv_client::{CheckLevel, Snapshot, Transaction, TransactionClient, TransactionOptions};

use crate::error::AppError;
use crate::repositories::Storage;
use keys::LockScope;
use kv::LazyTxn;

/// 競合・ロック待ちでやり直す上限。
const MAX_ATTEMPTS: usize = 20;

/// 競合でやり直すときの最初の待ち時間。以降は試行ごとに倍にする。
///
/// 待たずに `continue` すると、混み合っている相手へ PD 往復を伴う再挑戦を
/// 詰めて投げることになり、競合をさらに悪化させる。
const RETRY_BACKOFF_BASE: std::time::Duration = std::time::Duration::from_millis(5);
/// 待ち時間の上限。
///
/// **やり直しは書き込みクロージャを丸ごと実行し直す。** 大きな書き込みではその 1 回が
/// 秒の単位になるので、上限が小さいと「重い試行を隙間なく投げ続ける」ことになり、
/// 詰まっているクラスタをこちらから叩き続ける形になる。
///
/// 目安として、tikv-client がロックへ与える寿命は 20 秒（`MAX_TTL`）。そこを超えて
/// 生き延びられないトランザクションは何度やっても通らないので、上限はその手前まで
/// 伸ばしておき、混雑が収まる時間を与えるほうが早く収束する。
const RETRY_BACKOFF_MAX: std::time::Duration = std::time::Duration::from_millis(2000);

/// 待ち時間へ加える揺らぎの割合（±20%）。
///
/// 倍々に伸びるだけの待ち時間だと、同じ相手とぶつかった複数のインスタンスが
/// **揃った間隔でやり直し続ける**。TiKV の悲観ロックは待たずに即エラーを返す設定
/// （tikv-client が `wait_timeout = 0` を固定）なので、競合の解消はこちらの
/// やり直しに委ねられており、足並みが揃うと competing herd がそのまま持続する。
/// 揺らぎを入れて、ぶつかった側が別々のタイミングへばらけるようにする。
const RETRY_BACKOFF_JITTER_PERCENT: u64 = 20;

/// 競合でやり直す前に待つ。
///
/// 4 か所から呼ばれるので、待ち方と回数の数え方が散らばらないよう 1 つにまとめてある
/// （散らすと、片方だけ `last_error` を残し忘れるといった食い違いが起きる）。
async fn back_off(conflicts: &mut u32, reason: AppError, last_error: &mut Option<AppError>) {
    *last_error = Some(reason);
    tokio::time::sleep(retry_backoff(*conflicts)).await;
    *conflicts += 1;
}

/// `attempt` 回目の競合に対する待ち時間。
///
/// 打ち切りは `RETRY_BACKOFF_MAX` 側で行う。指数の頭を抑えるのは桁溢れを防ぐためだけで、
/// **上限より手前で頭打ちにならない**ように余裕を持たせてある（ここが上限より小さいと、
/// 定数を上げても実際の待ち時間が伸びない）。
fn retry_backoff(attempt: u32) -> std::time::Duration {
    let base = RETRY_BACKOFF_BASE
        .saturating_mul(1u32 << attempt.min(16))
        .min(RETRY_BACKOFF_MAX);
    apply_jitter(base)
}

/// 待ち時間に ±[`RETRY_BACKOFF_JITTER_PERCENT`]% の揺らぎを与える。
fn apply_jitter(base: std::time::Duration) -> std::time::Duration {
    let millis = base.as_millis() as u64;
    let span = millis * RETRY_BACKOFF_JITTER_PERCENT / 100;
    if span == 0 {
        // 揺らぎが 1ms 未満になる短さでは、丸めると意味を持たない。
        return base;
    }
    let mut bytes = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut bytes);
    // [millis - span, millis + span] の一様分布。
    let offset = u64::from_le_bytes(bytes) % (span * 2 + 1);
    std::time::Duration::from_millis(millis - span + offset)
}

fn to_app_error(err: tikv_client::Error) -> AppError {
    AppError::StorageError(err.to_string())
}

// --- トランザクションの開き方 ---
//
// tikv-client の既定は [`CheckLevel::Panic`] で、コミットも rollback もされないまま
// [`Transaction`] が drop されると panic する。HTTP ハンドラの Future はクライアント
// 切断で途中破棄されうるため、この経路は通常運用で踏まれる。リリースビルドは
// `panic = "abort"` なので、踏めばプロセスごと落ちる。
//
// そこでこのバックエンドは `begin_optimistic` / `begin_pessimistic` を直接使わず、
// 必ず下の 2 つを通して開く。破棄の後始末は [`rollback_in_background`] が引き受け、
// それでも取り残されたものだけが警告として残るようにしてある。

/// 読み取りトランザクションの設定。
fn read_options() -> TransactionOptions {
    TransactionOptions::new_optimistic().drop_check(CheckLevel::Warn)
}

/// 明示ロック専用トランザクションの設定。**必ず悲観**。
///
/// [`write_options`] と統合してはならない。楽観の `lock_keys` は RPC を出さず
/// ローカルバッファへ印を付けるだけで、検査はコミット時に行われる。このトランザクションは
/// 必ず rollback で終わるので、検査は永久に行われず**排他が黙って消える**
/// （モジュール冒頭のカタログ経路の節を参照）。
fn lock_options() -> TransactionOptions {
    TransactionOptions::new_pessimistic().drop_check(CheckLevel::Warn)
}

/// 作業（書き込み）トランザクションの設定。
///
/// **悲観**。一度は楽観へ寄せたが、競合の濃いワークロードで書き込みが落ちたため戻した
/// （理由と実測は `data.rs` の書き込みの節）。[`lock_options`] と分けたままにしてあるのは、
/// 将来また作業側だけを楽観へ寄せられるようにするため――そのときロック側まで
/// 巻き込まないことが要点で、テストがそれを固定している。
///
/// `one_pc` は 1 回の prewrite でコミットまで済ませる最適化。効くのは変更が
/// **1 リージョンに収まる**ときだけで、跨ぐ場合は tikv-client が自動的に 2PC へ落とす。
/// 収まったのに TiKV が断った場合は [`tikv_client::Error::OnePcFailure`] になる――
/// **フォールバックは実装されていない**ので、呼び出し側が 2PC でやり直す
/// （[`Storage::write`] を参照）。
fn write_options(one_pc: bool) -> TransactionOptions {
    let options = TransactionOptions::new_pessimistic().drop_check(CheckLevel::Warn);
    if one_pc { options.try_one_pc() } else { options }
}

/// 破棄されるトランザクションを、バックグラウンドで rollback する。
///
/// 悲観ロックを握ったまま消えると、ロックの TTL が切れるまで同じシャードへの
/// 書き込みが止まる。切断のたびにそれが起きると、詰まりが次の要求へ連鎖するので、
/// Future が途中で捨てられた場合でもロックだけは返しにいく。
///
/// ランタイムが既に無い（プロセス終了時など）なら諦めてそのまま捨てる。
fn rollback_in_background(mut txn: Transaction) {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                let _ = txn.rollback().await;
            });
        }
        Err(_) => drop(txn),
    }
}

/// ロックが足りないので、この試行を捨ててやり直すことを示すマーカー。
///
/// [`AppError`] とは別の型にしてあるのが要点で、[`TikvWrite::require_lock`] の戻り値を
/// 見ればそれが「アプリケーションの失敗」ではなく「制御用の巻き戻し」だと型で判る。
/// 操作側は `?` で伝播させるだけでよく、その際 [`From`] でアプリケーションエラーへ落ちる。
///
/// [`Storage::write`] はクロージャの結果を見る**前に**ロック充足を確認するので、
/// 変換後の値が呼び出し元へ届くことはない。届いたとすればそれは実装のバグであり、
/// メッセージからそう判るようにしてある。
#[derive(Debug, Clone, Copy)]
pub(crate) struct NeedsRestart;

impl From<NeedsRestart> for AppError {
    fn from(_: NeedsRestart) -> Self {
        AppError::InternalError(
            "lock declaration escaped the write retry loop (this is a bug in the tikv backend)"
                .to_string(),
        )
    }
}

/// TiKV が「やり直せば通る」種類の失敗を返したか。
///
/// ロック保持者がコミットすると待機側は書き込み競合として弾かれる。tikv-client は
/// これを内部でリトライせず呼び出し側へ委ねるため、ここで判定して自分でやり直す。
/// こうすることで「書き込みは待たされても失敗しない」という LMDB の性質が
/// 呼び出し側から見て保たれる。
///
/// 判定は `KeyError` のフィールドを直接見る。Debug 文字列の部分一致だと、
/// 利用者が付けた名前がたまたま一致して誤ってやり直したり、tikv-client 側の
/// 文言変更で黙って「待てば通る失敗」が 500 に化けたりする。
fn is_retryable(err: &tikv_client::Error) -> bool {
    use tikv_client::Error;

    match err {
        // `conflict` は書き込み競合（悲観トランザクションでの `PessimisticRetry` を含む）、
        // `deadlock` は TiKV のデッドロック検出、`retryable` は
        // 「クライアントはトランザクションをやり直してよい」という明示の指示。
        Error::KeyError(key_error) => {
            key_error.conflict.is_some()
                || key_error.deadlock.is_some()
                || !key_error.retryable.is_empty()
        }
        // 他者のロックが載ったキーへ prewrite しようとして、解決を待ちきれなかった。
        //
        // **これは「今その相手が握っている」という意味でしかない。** tikv-client は
        // `KeyIsLocked` を受けると `resolve_lock` で解決を試み、相手がまだ生きている
        // うちは短い backoff（2ms〜500ms を 10 回）で粘るが、それを使い切ると
        // この失敗になる。競合が濃いときほど当たる。
        //
        // 楽観トランザクションではロックに当たるのが prewrite なので、競合の signal が
        // ここへ出る。悲観だった頃は `pessimistic_lock` の段で `conflict` として
        // 上の分岐に落ちていたため、この変換を書き忘れると**混雑時だけ 500 になる**。
        Error::ResolveLockError(_) => true,
        Error::MultipleKeyErrors(errors) | Error::ExtractedErrors(errors) => {
            errors.iter().any(is_retryable)
        }
        Error::PessimisticLockError { inner, .. } => is_retryable(inner),
        // `UndeterminedError` はコミットの成否が不明なので、やり直すと二重適用に
        // なりうる。ここでは拾わず、呼び出し元へそのまま伝える。
        _ => false,
    }
}

/// 1PC を断られたか。
///
/// tikv-client は 1 リージョンに収まった変更に対して TiKV が 1PC を拒んだとき、
/// **2PC へフォールバックせず**この失敗を返す。やり直せば通るが、同じ条件なら
/// また断られるので、**次の試行では 1PC を諦める**必要がある（[`is_retryable`] と
/// 分けてあるのはそのため）。
fn is_one_pc_failure(err: &tikv_client::Error) -> bool {
    matches!(err, tikv_client::Error::OnePcFailure)
}

/// TiKV バックエンドのハンドル。複製してもクラスタへの接続は共有される。
///
/// 構築は `init.rs`（[`TikvDb::connect`]）が担う。
#[derive(Clone)]
pub struct TikvDb {
    pub(super) client: Arc<TransactionClient>,
}

/// 保持中のロック。解放は必ず rollback で行う。
///
/// 中身が [`Option`] なのは、[`release`](Self::release) を通らずに破棄された場合でも
/// [`Drop`] がロックを返しにいけるようにするため。
struct LockGuard {
    txn: Option<Transaction>,
}

impl LockGuard {
    /// ロックキーをまとめて取得する。
    ///
    /// `keys` が [`BTreeSet`] なのは偶然ではない。反復順（＝バイト昇順）が
    /// そのままデッドロック回避に必要な全順序になる（モジュール冒頭を参照）。
    /// まとめて渡すのは、`lock_keys` がリージョンごとに 1 リクエストへ束ねるため。
    /// 1 キーずつ呼ぶとロック数だけ往復が増える。
    async fn acquire(
        client: &TransactionClient,
        keys: &BTreeSet<Vec<u8>>,
    ) -> Result<Self, tikv_client::Error> {
        // **悲観**で開くこと（[`lock_options`] の注記を参照）。
        let mut txn = client.begin_with_options(lock_options()).await?;
        if let Err(e) = txn.lock_keys(keys.iter().cloned()).await {
            let _ = txn.rollback().await;
            return Err(e);
        }
        Ok(Self { txn: Some(txn) })
    }

    async fn release(mut self) {
        if let Some(mut txn) = self.txn.take() {
            let _ = txn.rollback().await;
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if let Some(txn) = self.txn.take() {
            rollback_in_background(txn);
        }
    }
}

/// ロックを保持していれば解放する。空集合の試行では取っていないので何もしない。
async fn release(guard: Option<LockGuard>) {
    if let Some(guard) = guard {
        guard.release().await;
    }
}

/// コミットし、**失敗したらロックを返してから**そのエラーを返す。
///
/// [`tikv_client::Transaction::commit`] は失敗しても自分では巻き戻さない。エラーを返して
/// 終わるだけで、prewrite が途中まで書いたロックはそのまま残る。ここで返しにいかないと、
/// ロックは TTL（3 秒〜最大 20 秒）が切れるまで居座る。
///
/// これは競合時に**雪だるま式に悪化する**。A の prewrite が競合で失敗してロックを残す →
/// B がそのロックに当たり、まだ生きているので `resolve_lock` が待たされ、backoff を
/// 使い切って失敗する → B もロックを残す → … と、詰まりが次の要求へ連鎖していく。
/// やり直す前にここで返しておけば、次の試行は自分が残した障害物に当たらない。
///
/// **`UndeterminedError` のときだけは巻き戻さない。** これは「主キーのコミットが
/// 成功したかどうか判らない」という意味で、実際には成功しているかもしれない。
/// 巻き戻すとコミット済みのトランザクションを取り消しかねないので、そのまま伝える。
async fn commit_or_release(txn: &mut Transaction) -> Result<(), tikv_client::Error> {
    match txn.commit().await {
        Ok(_) => Ok(()),
        Err(tikv_client::Error::UndeterminedError(inner)) => {
            Err(tikv_client::Error::UndeterminedError(inner))
        }
        Err(e) => {
            if let Err(rollback_error) = txn.rollback().await {
                // 巻き戻せなくても元の失敗を優先して伝える。ロックは TTL で消える。
                tracing::warn!("failed to release locks after a failed commit: {rollback_error}");
            }
            Err(e)
        }
    }
}

/// 開いていれば巻き戻す。一度も触れていない試行では開いていないので何もしない。
async fn rollback(txn: Option<Transaction>) {
    if let Some(mut txn) = txn {
        let _ = txn.rollback().await;
    }
}

/// 読み取りトランザクション。
///
/// `R` は読み取り元。通常の読み取りとクエリ実行器の入力源は開始タイムスタンプを
/// 固定した [`tikv_client::Snapshot`]、書き込みトランザクション上の読み取りは
/// [`kv::LazyTxn`] を使う。
pub struct TikvRead<'a, R = Snapshot> {
    pub(crate) txn: kv::Readers<R>,
    /// ストレージのハンドルより長生きしないことを型で示すだけのマーカー。
    pub(crate) _db: PhantomData<&'a TikvDb>,
}

impl TikvRead<'_, Snapshot> {
    /// `ts` の断面を読む読み取り元を作る。
    ///
    /// 読み取りにトランザクションを開かないのが要点。開くと開始で 1 往復、
    /// 後始末の rollback でもう 1 往復かかるが、読むだけならどちらも要らない。
    /// さらにタイムスタンプを手元に持てるので、同じ断面のスナップショットを
    /// 追加で開いて**チャンクを並行に読める**（[`kv::Readers`]）。
    pub(crate) fn at(client: Arc<TransactionClient>, ts: tikv_client::Timestamp) -> Self {
        Self {
            txn: kv::Readers::fanned_out(client, ts),
            _db: PhantomData,
        }
    }
}

/// 書き込みトランザクション。
///
/// 中身の [`LazyTxn`] は**最初に実際へ触れるまで開かない**。ロック宣言だけで
/// 巻き戻る 1 周目が、ネットワークに触れずに終わるようにするため。
pub struct TikvWrite<'a> {
    pub(crate) txn: kv::Readers<LazyTxn>,
    pub(crate) _db: PhantomData<&'a TikvDb>,
    /// この試行で保持しているロック。
    held: BTreeSet<Vec<u8>>,
    /// 操作が宣言したが、まだ保持していないロック。
    /// 空でなければ、この試行は巻き戻してやり直す。
    missing: BTreeSet<Vec<u8>>,
    /// 読んだ内容が古かったので、この試行を捨てて新しいスナップショットでやり直す。
    stale: bool,
}

impl TikvWrite<'_> {
    /// この操作が触る範囲を宣言する。**実際にデータへ触れる前**に呼ぶこと。
    ///
    /// まだ取得していないロックだった場合は [`NeedsRestart`] を返す。呼び出し側が
    /// `?` で伝播させれば、その試行は破棄され、宣言されたロックを揃えた状態で
    /// クロージャが最初から実行し直される。
    ///
    /// 宣言忘れをコンパイラに検出させるため、確認を戻り値に持たせている
    /// （フラグを別途調べる方式では、確認を書き忘れてもコンパイルが通ってしまう）。
    pub(crate) fn require_lock(&mut self, scope: LockScope, id: &[u8]) -> Result<(), NeedsRestart> {
        let key = keys::lock(scope, id);
        if self.held.contains(&key) {
            return Ok(());
        }
        self.missing.insert(key);
        Err(NeedsRestart)
    }

    /// 複数スコープをまとめて宣言する。
    ///
    /// 1 つずつ宣言してやり直すと 1 回の試行で 1 つしかロックを集められないので、
    /// 複数必要な操作はここで全部宣言してから戻る。取得順は集合が決めるので、
    /// 渡す順序は問わない（モジュール冒頭のデッドロックの節を参照）。
    pub(crate) fn require_locks<'k>(
        &mut self,
        scopes: impl IntoIterator<Item = (LockScope, &'k [u8])>,
    ) -> Result<(), NeedsRestart> {
        let mut missing_any = false;
        for (scope, id) in scopes {
            if self.require_lock(scope, id).is_err() {
                missing_any = true;
            }
        }
        if missing_any {
            return Err(NeedsRestart);
        }
        Ok(())
    }

    /// 読んだ木の形が古かったことを記録し、この試行を捨てさせる。
    ///
    /// 同じトランザクションの中で読み直しても意味がない。`get` は常に `start_ts` の
    /// スナップショットを読むので、何度試しても同じ古い答えが返るためである。
    /// 新しいトランザクションを開けば `start_ts` が他者のコミットより後になり、
    /// 必ず前進する。
    pub(crate) fn mark_stale(&mut self) -> NeedsRestart {
        self.stale = true;
        NeedsRestart
    }

    /// この試行を捨ててやり直す必要があるか。
    ///
    /// 理由は 2 つある。宣言されたロックが足りない（`missing`）か、読んだ内容が
    /// 古かった（`stale`）か。どちらも「コミットしてはいけない」点では同じ。
    fn needs_restart(&self) -> bool {
        !self.missing.is_empty() || self.stale
    }
}

impl Storage for TikvDb {
    type Read<'a> = TikvRead<'a, Snapshot>;
    type Write<'a> = TikvWrite<'a>;
    /// 断面は開始タイムスタンプそのもの。各読み取りはこの時刻の
    /// [`tikv_client::Snapshot`] を開く（`query_source.rs`）。
    type QuerySnapshot = tikv_client::Timestamp;

    #[tracing::instrument(skip_all)]
    async fn query_snapshot(&self) -> Result<Self::QuerySnapshot, AppError> {
        self.client.current_timestamp().await.map_err(to_app_error)
    }

    #[tracing::instrument(skip_all)]
    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        // 読み取りはロックを取らない。スナップショットから読むだけなので
        // 書き込みをブロックせず、書き込みにもブロックされない（LMDB と同じ）。
        //
        // トランザクションではなくスナップショットを開く（[`TikvRead::at`] を参照）。
        let ts = self
            .client
            .current_timestamp()
            .await
            .map_err(to_app_error)?;
        let r = TikvRead::at(self.client.clone(), ts);
        f(&r).await
    }

    #[tracing::instrument(skip_all)]
    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        let mut locks: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut last_error: Option<AppError> = None;
        // 競合でのやり直しだけを数える。ロック不足での巻き戻しは待つ理由がない
        // （相手を待っているのではなく、必要な範囲が判っただけなので）。
        let mut conflicts: u32 = 0;
        // 1PC を試すか。断られたら諦めて 2PC でやり直す（[`is_one_pc_failure`]）。
        // 試行をまたいで持ち回るのが要点で、そうしないと断られ続ける構成で
        // 毎回 1 回ぶん無駄な prewrite を投げ、上限まで失敗し続ける。
        let mut one_pc = true;

        for _ in 0..MAX_ATTEMPTS {
            // 1. ロックを取得する。初回は集合が空なので、そもそもトランザクションを
            //    開かない（空の `lock_keys` は何もしないのに、開くだけで PD から
            //    タイムスタンプを取ることになる）。
            let guard = if locks.is_empty() {
                None
            } else {
                match LockGuard::acquire(&self.client, &locks).await {
                    Ok(guard) => Some(guard),
                    Err(e) if is_retryable(&e) => {
                        back_off(&mut conflicts, to_app_error(e), &mut last_error).await;
                        continue;
                    }
                    Err(e) => return Err(to_app_error(e)),
                }
            };

            // 2. 作業トランザクションはロックを保持した状態で開く。実際に開くのは
            //    クロージャが最初にデータへ触れたときで（[`LazyTxn`]）、そこで
            //    start_ts が確定する。ロック取得より後になるので、前任者のコミットが
            //    必ず見える。
            let mut w = TikvWrite {
                txn: kv::Readers::new(LazyTxn::new(self.client.clone(), one_pc)),
                _db: PhantomData,
                held: locks.clone(),
                missing: BTreeSet::new(),
                stale: false,
            };

            // やり直しに備えて複製を渡す。`f` 自体は次の試行のために残す。
            let result = f.clone()(&mut w).await;
            // やり直しの要否は**結果より先**に確認する。こうしておけば、クロージャが
            // `NeedsRestart` 由来のエラーを握り潰して `Ok` を返しても、その試行は
            // 確実に捨てられる（＝宣言し忘れた範囲や古い読みのまま commit されない）。
            let restart = w.needs_restart();
            let newly_required = std::mem::take(&mut w.missing);
            let was_stale = w.stale;
            let mut lazy = w.txn.into_inner();

            // 溜めた変更をここで初めて送る（[`LazyTxn::flush`]）。捨てる試行では
            // 送らない――送らなければロックも MVCC のバージョンも作られずに済む。
            let flushed = if restart || result.is_err() {
                Ok(())
            } else {
                lazy.flush().await
            };

            // ロックの取得はクロージャの内側で起きるので、競合はアプリケーションエラーの
            // 形で返ってくる。やり直せる失敗かどうかは作業トランザクションが覚えている。
            let conflicted = lazy.conflicted();
            // 一度もデータへ触れていなければトランザクションは開いていない。
            let txn = lazy.into_opened();

            // 3. やり直しが要るなら、この試行は捨てる。
            if restart {
                rollback(txn).await;
                release(guard).await;
                locks.extend(newly_required);
                // 古い読みが原因なら、他者のコミットが見えるようになるまで少し待つ。
                // ロック不足が原因の場合は待つ理由がない（競合ではなく、必要な範囲が
                // 判っただけなので、次の周回は必ず前進する）。
                if was_stale {
                    let reason = AppError::Conflict(
                        "read a stale view of the shard tree; retrying with a newer snapshot"
                            .to_string(),
                    );
                    back_off(&mut conflicts, reason, &mut last_error).await;
                }
                continue;
            }

            // 送信そのものが失敗したなら、コミットへは進まない。
            if let Err(e) = flushed {
                rollback(txn).await;
                release(guard).await;
                if is_retryable(&e) {
                    back_off(&mut conflicts, to_app_error(e), &mut last_error).await;
                    continue;
                }
                return Err(to_app_error(e));
            }

            let outcome = match result {
                Ok(value) => match txn {
                    Some(mut txn) => commit_or_release(&mut txn).await.map(|_| value),
                    // 何も書かずに終わった（読み取りだけ、あるいは即座に確定した）。
                    None => Ok(value),
                },
                Err(app_err) => {
                    // クロージャが失敗したらコミットしない。ロックは必ず解放する。
                    rollback(txn).await;
                    release(guard).await;
                    if conflicted {
                        // 競合で弾かれただけなので、新しいトランザクションでやり直す。
                        // 呼び出し側から見て「書き込みは待たされても失敗しない」を保つ。
                        back_off(&mut conflicts, app_err, &mut last_error).await;
                        continue;
                    }
                    return Err(app_err);
                }
            };

            release(guard).await;

            match outcome {
                Ok(value) => return Ok(value),
                // 1PC を断られただけ。競合ではないので待たず、2PC で即やり直す。
                Err(e) if is_one_pc_failure(&e) => {
                    tracing::debug!("TiKV declined one-phase commit; retrying with two-phase");
                    one_pc = false;
                    last_error = Some(to_app_error(e));
                    continue;
                }
                Err(e) if is_retryable(&e) => {
                    back_off(&mut conflicts, to_app_error(e), &mut last_error).await;
                    continue;
                }
                Err(e) => return Err(to_app_error(e)),
            }
        }

        // ここへ来る理由は 2 つある。競合し続けた（`last_error` あり）か、
        // ロック宣言が収束しなかった（実装のバグ）か。区別できるようにしておく。
        Err(last_error.unwrap_or_else(|| {
            AppError::Conflict(format!(
                "write did not settle within {MAX_ATTEMPTS} attempts ({} lock(s) held at the end)",
                locks.len()
            ))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LockScope` の判別値が「データベース → テーブル → ユーザー」の順に並ぶこと。
    ///
    /// ロックキーのバイト昇順がそのまま取得順になるので、この並びが崩れると
    /// デッドロック回避の前提が壊れる。
    #[test]
    fn lock_scopes_sort_into_the_hierarchy_order() {
        let db = keys::lock(LockScope::Database, b"z-database");
        let user = keys::lock(LockScope::User, b"a-user");

        // id の中身に関わらず、スコープの順序が先に効く。
        assert!(db < user);

        let ordered: Vec<_> = BTreeSet::from([user.clone(), db.clone()])
            .into_iter()
            .collect();
        assert_eq!(ordered, vec![db, user]);
    }

    /// バックオフが上限を超えず、かつ毎回同じ値にならないこと。
    ///
    /// 揃った間隔でやり直すと、ぶつかった相手と足並みが揃ったままになる。
    #[test]
    fn retry_backoff_grows_within_bounds_and_varies() {
        // 上限を超えない（揺らぎの上振れを含めて）。
        let ceiling =
            RETRY_BACKOFF_MAX + RETRY_BACKOFF_MAX * RETRY_BACKOFF_JITTER_PERCENT as u32 / 100;
        for attempt in 0..32 {
            assert!(
                retry_backoff(attempt) <= ceiling,
                "attempt={attempt} で上限を超えた"
            );
        }

        // 十分に伸びた段階では、毎回同じ値にならない。
        let samples: std::collections::BTreeSet<_> = (0..32).map(|_| retry_backoff(16)).collect();
        assert!(
            samples.len() > 1,
            "揺らぎが効いておらず、待ち時間が毎回同じになっている"
        );

        // 揺らぎは ±20% の範囲に収まる。
        let base = RETRY_BACKOFF_MAX.as_millis() as u64;
        let span = base * RETRY_BACKOFF_JITTER_PERCENT / 100;
        for sample in samples {
            let millis = sample.as_millis() as u64;
            assert!(
                millis >= base - span && millis <= base + span,
                "揺らぎが想定範囲を外れている: {millis}ms"
            );
        }
    }

    /// 競合由来の失敗がやり直し対象に分類されること。
    ///
    /// 楽観トランザクションでは他者のロックに当たるのが prewrite なので、競合の signal は
    /// [`tikv_client::Error::ResolveLockError`] として出る。ここが漏れていると、
    /// **混雑したときだけ 500 が返る**（空いていれば当たらないので気づけない）。
    #[test]
    fn contention_failures_are_classified_as_retryable() {
        use tikv_client::Error;

        // 他者がロックを握っていて解決を待ちきれなかった。待てば通る。
        assert!(
            is_retryable(&Error::ResolveLockError(Vec::new())),
            "ロック解決の失敗がやり直し対象から漏れている"
        );
        // 入れ子（リージョンを跨いだ prewrite などでまとめられた場合）でも拾う。
        assert!(is_retryable(&Error::ExtractedErrors(vec![
            Error::ResolveLockError(Vec::new()),
        ])));

        // コミットの成否が不明なものはやり直してはならない（二重適用になりうる）。
        assert!(!is_retryable(&Error::UndeterminedError(Box::new(
            Error::ResolveLockError(Vec::new())
        ))));

        // 1PC 拒否は競合ではないので、こちらの分類には入れない
        // （待たずに 2PC でやり直す。`is_one_pc_failure` を参照）。
        assert!(!is_retryable(&Error::OnePcFailure));
        assert!(is_one_pc_failure(&Error::OnePcFailure));
    }

    /// ロック専用トランザクションが**悲観のまま**であること。
    ///
    /// これが楽観になると `lock_keys` は RPC を出さずローカルバッファへ印を付けるだけになり、
    /// 検査はコミット時に回る。[`LockGuard`] は必ず rollback で終わるので検査は永久に
    /// 行われず、**カタログ操作の排他が黙って消える**。
    ///
    /// 消えても単体テストは通ってしまう（競合を能動的に起こさないため）。壊れ方は
    /// 「複数インスタンスでだけ、範囲スキャンから導く操作がファントムを起こす」形なので、
    /// 気づくのは本番になる。だからここで型として固定しておく。
    ///
    /// 作業側（[`write_options`]）は性能の都合で楽観へ振れうる。そのときに
    /// ロック側まで巻き込まないことが、この検査の目的である。
    #[test]
    fn lock_transactions_stay_pessimistic() {
        // 悲観／楽観の別は `TransactionOptions` の外から読めないので、
        // 既知の構成と突き合わせて判定する。
        let pessimistic = TransactionOptions::new_pessimistic().drop_check(CheckLevel::Warn);
        assert_eq!(lock_options(), pessimistic, "ロック専用は悲観であること");

        // 作業側は今も悲観だが、**それに依存して同一視してはならない**。
        // 作業側だけを楽観へ寄せる変更が入っても、ロック側は悲観のままである必要がある。
        let optimistic = TransactionOptions::new_optimistic().drop_check(CheckLevel::Warn);
        assert_ne!(
            lock_options(),
            optimistic,
            "ロック専用が楽観になっている。lock_keys が RPC を出さなくなり排他が消える"
        );

        assert_ne!(
            write_options(true),
            write_options(false),
            "try_one_pc が反映されていない"
        );
    }

    #[test]
    fn restart_marker_is_labelled_as_a_bug_when_converted() {
        let err: AppError = NeedsRestart.into();
        let AppError::InternalError(message) = err else {
            panic!("NeedsRestart は InternalError へ落ちるべき");
        };
        assert!(message.contains("bug"), "実装バグと判る文言であること");
    }
}
