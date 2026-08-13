//! トランザクション上の素の KV 操作と、シャード値のフレーム。
//!
//! 読み取りは [`TikvRead`](super::TikvRead) と [`TikvWrite`](super::TikvWrite) の
//! どちらからも必要なので、読み取り元だけを受け取る自由関数として置いている。
//!
//! # なぜ読み取りが [`Reader`] で抽象化されているか
//!
//! 書き込みトランザクション上の読み取りは [`tikv_client::Transaction`]、通常の読み取りと
//! クエリ実行器の入力源（`query_source.rs`）は**開始タイムスタンプを固定した**
//! [`tikv_client::Snapshot`] を使う。両者は読み取り系のシグネチャが同一なので、
//! ここで 1 つの trait にまとめ、下の自由関数を両方で使い回す。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use kasane_logic::FlexId;
use rustc_hash::FxHashSet;
pub(super) use tikv_client::proto::kvrpcpb::Mutation;
use tikv_client::proto::kvrpcpb::{Assertion, Op};
use tikv_client::{BoundRange, Snapshot, Timestamp, Transaction, TransactionClient};
use tokio::sync::{Mutex, MutexGuard};

use super::keys;
use crate::error::AppError;
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::ShardEntry;

/// 1 回のスキャンで取り出す件数。TiKV は 1 リクエストの上限があるため分割して読む。
const SCAN_BATCH: u32 = 512;

/// 1 リクエストへ載せるキー数の上限。
///
/// `batch_get` / `batch_get_for_update` / `batch_mutate` は渡されたキーを
/// TiKV のリージョンごとに束ねる。同じテーブルのシャードキーは連続しているため、
/// 大量のキーがひとつのリージョンに集まりやすく、そのまま渡すと gRPC の
/// メッセージサイズ上限に触れたり、応答をまとめて保持する分だけメモリが跳ねる。
///
/// 呼び出し側で数を見積もるのは難しい（木の形と対象領域の広さで変わり、
/// `route_leaves_for_range` のように上限を持たない経路もある）ので、
/// **ここで一律に区切る**。分割してもキーの昇順は保たれるので、
/// ロック取得順に依存するデッドロック回避の前提は崩れない。
const BATCH_KEYS: usize = 1024;

/// 並行に開く追加スナップショットの上限（[`Readers::fanned_out`]）。
///
/// 1 チャンクが既にリージョン単位で内部並行化されているので、ここを大きくしても
/// 増えるのは同時に飛ぶリクエスト数と手元に載る応答の量だけになる。
const MAX_FANOUT: usize = 8;

// --- 読み取りの抽象 ---

/// スナップショット読み取りができるもの（[`Transaction`] と [`Snapshot`]）。
///
/// 下の自由関数がこの trait 越しに動くので、呼び出し側は「どちらのスナップショットか」
/// を意識せずに同じ読み取り経路を通れる。
///
/// `pub` なのは [`TikvRead`](super::TikvRead) の型引数の境界に現れるためだけで、
/// バックエンドの内部事情。`kv` モジュール自体が非公開なので外からは名指しできない。
// `async fn` の Future に `Send` を課さないのは `traits/storage.rs` と同じ理由。
#[allow(async_fn_in_trait)]
pub trait Reader {
    async fn read_one(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, tikv_client::Error>;

    async fn read_many(
        &mut self,
        keys: Vec<Vec<u8>>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error>;

    async fn read_range(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error>;

    async fn read_range_keys(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<Vec<u8>>, tikv_client::Error>;

    /// `start`（含む）〜`end`（排他、`None` なら終端まで）に入る**まだ送っていない変更**を
    /// キー昇順で返す（`None` は削除）。
    ///
    /// 点読み（[`read_one`](Self::read_one) / [`read_many`](Self::read_many)）は自分の変更を
    /// 自分で重ねられるが、範囲読みはそうはいかない。[`scan_prefix`] のような「読み切る」
    /// 関数は**取得件数が上限に満たないこと**を終端の判定に使っているので、その途中で
    /// 件数を増減させると読み切る前に止まったり同じ場所を読み直したりする。そこで
    /// 読み切ったあとに重ねられるよう、変更の側をここから取り出せるようにしてある。
    ///
    /// 溜める仕組みを持たない読み取り元（[`Transaction`] / [`Snapshot`]）では常に空。
    fn staged_range(&self, _start: &[u8], _end: Option<&[u8]>) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
        Vec::new()
    }
}

/// [`Transaction`] と [`Snapshot`] は読み取り系のシグネチャが完全に一致するので、
/// 実装は 1 つの雛形から生成する。
macro_rules! impl_reader {
    ($target:ty) => {
        impl Reader for $target {
            async fn read_one(
                &mut self,
                key: Vec<u8>,
            ) -> Result<Option<Vec<u8>>, tikv_client::Error> {
                self.get(key).await
            }

            async fn read_many(
                &mut self,
                keys: Vec<Vec<u8>>,
            ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
                Ok(self
                    .batch_get(keys)
                    .await?
                    .map(|kv| (Vec::from(kv.0), kv.1))
                    .collect())
            }

            async fn read_range(
                &mut self,
                range: BoundRange,
                limit: u32,
            ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
                Ok(self
                    .scan(range, limit)
                    .await?
                    .map(|kv| (Vec::from(kv.0), kv.1))
                    .collect())
            }

            async fn read_range_keys(
                &mut self,
                range: BoundRange,
                limit: u32,
            ) -> Result<Vec<Vec<u8>>, tikv_client::Error> {
                Ok(self.scan_keys(range, limit).await?.map(Vec::from).collect())
            }
        }
    };
}

impl_reader!(Transaction);
impl_reader!(Snapshot);

// --- 読み取り元 ---

/// 読み取り／書き込みの実体と、並行読みのための追加スナップショットの作り方。
///
/// [`Reader`] は `&mut self` を要求する（トランザクションもスナップショットも内部に
/// 可変状態を持つ）ので、1 つの実体では**チャンクを逐次にしか読めない**。木の同じ深さで
/// [`BATCH_KEYS`] を超えるキーを引く場面はテーブルが大きいほど普通に起きるため、
/// そこが往復の直列鎖になる。
///
/// 断面がタイムスタンプで決まる読み取り（[`fanned_out`](Self::fanned_out)）なら、
/// **同じタイムスタンプのスナップショットをいくつでも開ける**。追加のスナップショットは
/// 同じ断面を読むので、どのチャンクをどれで読んでも結果は変わらない。
///
/// 書き込みトランザクション上の読み取りには追加スナップショットを持たせない。
/// そちらは**まだ送っていない自分の変更**を重ねて返す必要があり（[`LazyTxn`] の
/// `Reader` 実装）、別の実体から読むとその重ね合わせが効かないため。
pub struct Readers<R> {
    primary: Mutex<R>,
    fanout: Option<Fanout>,
}

/// 同じ断面のスナップショットを追加で開くための材料。
struct Fanout {
    client: Arc<TransactionClient>,
    ts: Timestamp,
}

impl Fanout {
    fn open(&self) -> Snapshot {
        self.client.snapshot(self.ts.clone(), super::read_options())
    }
}

impl<R> Readers<R> {
    /// 実体 1 つだけの読み取り元（並行読みはしない）。
    pub(super) fn new(reader: R) -> Self {
        Self {
            primary: Mutex::new(reader),
            fanout: None,
        }
    }

    pub(super) fn into_inner(self) -> R {
        self.primary.into_inner()
    }

    async fn lock(&self) -> MutexGuard<'_, R> {
        self.primary.lock().await
    }
}

impl Readers<Snapshot> {
    /// `ts` の断面を読むスナップショットと、同じ断面を追加で開く手段を持つ読み取り元。
    pub(super) fn fanned_out(client: Arc<TransactionClient>, ts: Timestamp) -> Self {
        let primary = client.snapshot(ts.clone(), super::read_options());
        Self {
            primary: Mutex::new(primary),
            fanout: Some(Fanout { client, ts }),
        }
    }
}

// --- 作業トランザクション（遅延生成） ---

/// 書き込み側のトランザクション。**最初に実際へ触れるまで開かない。**
///
/// 書き込みクロージャの 1 周目は、必要なロックを宣言した時点で巻き戻される
/// （`super::mod` の「必要なロックをどう知るか」）。その周回でトランザクションを
/// 開いてしまうと、何もせずに捨てるためだけに PD からタイムスタンプを取ることになる。
/// 遅延生成にすると、1 周目はネットワークに一切触れずに終わる。
///
/// 開くのが遅れることは正しさを損なわない。むしろ「ロックを取ってから
/// `start_ts` を確定させる」という不変条件を、より遅い側へ寄せて強めている。
pub struct LazyTxn {
    client: Arc<TransactionClient>,
    txn: Option<Transaction>,
    /// 1PC を試すか（`super::write_options` を参照）。断られた場合のやり直しは
    /// `Storage::write` が判断するので、ここは受け取った設定を持ち回るだけ。
    one_pc: bool,
    /// まだ TiKV へ送っていない変更（`None` は削除）。
    ///
    /// **書き込みは全部ここへ溜め、コミットの直前に 1 回だけ流す**
    /// （[`flush`](Self::flush)）。呼び出しのたびに `batch_mutate` を投げると、
    /// 触れたリーフの数だけリクエストが直列に積み上がる。1 回にまとめれば、
    /// 書き込みの往復数はリーフ数に依存しなくなる。
    ///
    /// [`BTreeMap`] なのは偶然ではない。反復順（＝キーのバイト昇順）で送るので
    /// キーが連続し、リージョン跨ぎのアクセスが減る。同じキーへの重複書き込みも
    /// 最後の 1 つに畳まれる。
    ///
    /// この台帳は**読み取りに自分の書き込みを見せる**用途も兼ねる（[`LazyTxn`] の
    /// `Reader` 実装）。溜めている間は TiKV 側のバッファが空なので、二重に持つことはない。
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    /// 競合由来の失敗を受け取ったか。
    ///
    /// ロックの取得はこのトランザクションの**内側**で起きるので、競合は
    /// `Storage::write` から見ると「クロージャが返したアプリケーションエラー」に
    /// 見えてしまう。それではやり直しの判断ができないため、ここに記録して
    /// `Storage::write` が拾えるようにする。
    conflicted: bool,
}

impl LazyTxn {
    pub(super) fn new(client: Arc<TransactionClient>, one_pc: bool) -> Self {
        Self {
            client,
            txn: None,
            one_pc,
            pending: BTreeMap::new(),
            conflicted: false,
        }
    }

    /// tikv のエラーを記録しつつそのまま返す。
    ///
    /// 競合由来なら印を付けておき、`Storage::write` に新しいトランザクションで
    /// やり直させる。
    ///
    /// **このトランザクションが出す失敗は読み書きを問わずここを通すこと。** 通し忘れると
    /// その失敗は `Storage::write` から「ただのアプリケーションエラー」に見え、
    /// やり直されずに 500 として出ていく。読み取り側（[`Reader`] 実装）も同様で、
    /// 木の降下が他者のロックに当たる経路があるため対象になる。
    fn note(&mut self, err: tikv_client::Error) -> tikv_client::Error {
        if super::is_retryable(&err) {
            self.conflicted = true;
        }
        err
    }

    /// 競合でやり直せる失敗を受け取ったか。
    pub(super) fn conflicted(&self) -> bool {
        self.conflicted
    }

    async fn open(&mut self) -> Result<&mut Transaction, tikv_client::Error> {
        if self.txn.is_none() {
            self.txn = Some(
                self.client
                    .begin_with_options(super::write_options(self.one_pc))
                    .await?,
            );
        }
        Ok(self.txn.as_mut().expect("直前に開いた"))
    }

    /// 開いていたトランザクションを取り出す。一度も触れていなければ `None`。
    ///
    /// 取り出した後は [`Drop`] の対象が無くなるので、後始末は呼び出し側の責任になる。
    pub(super) fn into_opened(mut self) -> Option<Transaction> {
        self.txn.take()
    }

    /// 変更を台帳へ積む。**ここではネットワークに触れない**（[`pending`](Self::pending)）。
    fn stage(&mut self, key: Vec<u8>, value: Option<Vec<u8>>) {
        self.pending.insert(key, value);
    }

    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.stage(key, Some(value));
    }

    fn delete(&mut self, key: Vec<u8>) {
        self.stage(key, None);
    }

    fn mutate(&mut self, mutations: Vec<Mutation>) {
        for m in mutations {
            let value = (m.op != Op::Del as i32).then_some(m.value);
            self.stage(m.key, value);
        }
    }

    /// 溜めた変更をまとめて送り、悲観ロックを取る。**コミットの直前に 1 度だけ呼ぶ。**
    ///
    /// 全キーを 1 回の `batch_mutate` へ載せるので、ロック取得は
    /// 「TSO 1 回 + PessimisticLock 1 回」で済む（リージョン単位の分割は
    /// tikv-client が内部で並行に行う）。[`BATCH_KEYS`] で区切っても、
    /// [`BTreeMap`] 由来の昇順は保たれる。
    ///
    /// 捨てる試行では呼ばない。溜めたまま drop すれば、ロックも MVCC のバージョンも
    /// 一切作られないまま消える。
    pub(super) async fn flush(&mut self) -> Result<(), tikv_client::Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        // 台帳は空にする。ここから先の持ち主は TiKV 側のバッファなので、
        // 両方が同じバイト列を抱える瞬間を作らない。
        let staged = std::mem::take(&mut self.pending);
        let mut mutations: Vec<Mutation> = staged
            .into_iter()
            .map(|(key, value)| match value {
                Some(value) => put_mutation(key, value),
                None => delete_mutation(key),
            })
            .collect();

        while !mutations.is_empty() {
            let rest = mutations.split_off(mutations.len().min(BATCH_KEYS));
            let result = self.open().await?.batch_mutate(mutations).await;
            result.map_err(|e| self.note(e))?;
            mutations = rest;
        }
        Ok(())
    }

    /// キーをロックし、**そのロック時点の**値を返す。
    ///
    /// 通常の `get` はトランザクション開始時（`start_ts`）のスナップショットを読むため、
    /// 「ロックしてから読む」と、ロック取得までの間に他者がコミットした変更を見落とす。
    /// `batch_get_for_update` はロックと値の取得を 1 リクエストで行い、値は取り直した
    /// `for_update_ts` 時点のものになるので、read-modify-write が安全に組める。
    ///
    /// 存在しないキーも**ロックされる**（結果には現れない）。空の領域を他者が
    /// 埋めたり親へ畳んだりするのを防ぐのに、この性質を使っている。
    ///
    /// # 自分自身の書き込みを見せる
    ///
    /// このトランザクションの変更は [`pending`](Self::pending) に溜まっていて、
    /// コミットの直前まで TiKV へ送られない。したがってロック応答には自分の変更が
    /// 一切映らず、1 回の書き込みクロージャで同じリーフを 2 度触る操作
    /// （まとめて `data_insert` する、削除後に `try_merge_up` で読み直す等）が
    /// 自分の書き込みを失う。そこで手元の台帳を優先させる。
    ///
    /// （送った後であっても `batch_get_for_update` は TiKV 側のローカルバッファを
    /// 見ない――`get` は見るが、こちらはロック応答の値をそのまま返す――ので、
    /// どちらにせよこの重ね合わせは必要になる。）
    async fn lock_and_read(
        &mut self,
        mut keys: Vec<Vec<u8>>,
    ) -> Result<HashMap<Vec<u8>, Vec<u8>>, tikv_client::Error> {
        // キーは昇順で渡ってくる。分割しても順序は保たれるので、ロック取得順に
        // 依存するデッドロック回避の前提は崩れない。
        let mut locked: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        let all_keys = keys.clone();
        while !keys.is_empty() {
            let rest = keys.split_off(keys.len().min(BATCH_KEYS));
            let result = self.open().await?.batch_get_for_update(keys).await;
            let pairs = result.map_err(|e| self.note(e))?;
            locked.extend(pairs.into_iter().map(|kv| (Vec::from(kv.0), kv.1)));
            keys = rest;
        }

        // 自分が書いていないキーはロック応答のまま、書いたキーは手元の値で置き換える。
        for key in all_keys {
            match self.pending.get(&key) {
                Some(Some(value)) => {
                    locked.insert(key, value.clone());
                }
                Some(None) => {
                    locked.remove(&key);
                }
                None => {}
            }
        }
        Ok(locked)
    }
}

impl Drop for LazyTxn {
    /// クロージャの実行が途中で捨てられても、握った悲観ロックは返す
    /// （`super::rollback_in_background` の注記を参照）。
    fn drop(&mut self) {
        if let Some(txn) = self.txn.take() {
            super::rollback_in_background(txn);
        }
    }
}

/// 読み取りは**自分の未送信の変更を先に見る**。そして**失敗は必ず
/// [`note`](LazyTxn::note) を通す**。
///
/// 後者は競合の取りこぼしを防ぐためにある。木の降下も含めて、書き込みトランザクション上の
/// 読み取りは他者のロックに当たりうる。当たると tikv-client は `resolve_lock` で解決を
/// 試み、相手がまだ生きていれば backoff（合計 1.5 秒ほど）を使い切って
/// `ResolveLockError` を返す。**これは「今その相手が握っている」だけなのでやり直せば通る**
/// が、`note` を通さないと `Storage::write` は競合だと気づけず、1 回で 500 を返してしまう。
/// 混雑したときにだけ出るので、取りこぼしても空いていれば気づけない。
///
/// 変更はコミット直前まで手元に溜まる（[`LazyTxn::pending`]）ので、TiKV へ聞いても
/// 自分の書き込みは映っていない。1 つの書き込みクロージャが「書いてから読む」ことは
/// 普通にある（データベースを作ってすぐその中にテーブルを作る、など）ため、ここで
/// 重ねないと直前に自分が書いたものが見えなくなる。
impl Reader for LazyTxn {
    async fn read_one(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, tikv_client::Error> {
        if let Some(staged) = self.pending.get(&key) {
            return Ok(staged.clone());
        }
        let result = self.open().await?.read_one(key).await;
        result.map_err(|e| self.note(e))
    }

    async fn read_many(
        &mut self,
        keys: Vec<Vec<u8>>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        // 自分が触ったキーは手元で答え、残りだけを TiKV へ聞く。
        let mut staged = Vec::new();
        let mut ask = Vec::with_capacity(keys.len());
        for key in keys {
            match self.pending.get(&key) {
                Some(Some(value)) => staged.push((key, value.clone())),
                // 自分で消したキーは「存在しない」＝結果に現れない。
                Some(None) => {}
                None => ask.push(key),
            }
        }

        let mut out = if ask.is_empty() {
            Vec::new()
        } else {
            let result = self.open().await?.read_many(ask).await;
            result.map_err(|e| self.note(e))?
        };
        out.append(&mut staged);
        Ok(out)
    }

    /// 範囲読みは生のまま返す。重ねるのは呼び出し側（[`staged_range`](Reader::staged_range)）。
    async fn read_range(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        let result = self.open().await?.read_range(range, limit).await;
        result.map_err(|e| self.note(e))
    }

    async fn read_range_keys(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<Vec<u8>>, tikv_client::Error> {
        let result = self.open().await?.read_range_keys(range, limit).await;
        result.map_err(|e| self.note(e))
    }

    fn staged_range(&self, start: &[u8], end: Option<&[u8]>) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
        use std::ops::Bound;

        let lower = Bound::Included(start.to_vec());
        let upper = match end {
            Some(end) => Bound::Excluded(end.to_vec()),
            None => Bound::Unbounded,
        };
        self.pending
            .range((lower, upper))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

// --- 素の KV 操作 ---

pub(super) async fn get<R: Reader>(
    txn: &Readers<R>,
    key: Vec<u8>,
) -> Result<Option<Vec<u8>>, AppError> {
    let mut txn = txn.lock().await;
    txn.read_one(key).await.map_err(AppError::from)
}

pub(super) async fn put(
    txn: &Readers<LazyTxn>,
    key: Vec<u8>,
    value: Vec<u8>,
) -> Result<(), AppError> {
    txn.lock().await.put(key, value);
    Ok(())
}

pub(super) async fn delete(txn: &Readers<LazyTxn>, key: Vec<u8>) -> Result<(), AppError> {
    txn.lock().await.delete(key);
    Ok(())
}

/// 複数キーへの変更をまとめて適用する。
///
/// 実際の送信は [`LazyTxn::flush`] がコミット直前に 1 回だけ行う。ここでの「まとめて」は
/// **同じ台帳へ積む単位**という意味しか持たないが、呼び出し側から見た意味論
/// （この変更群は不可分に適用される）は変わらない。
pub(super) async fn mutate_many(
    txn: &Readers<LazyTxn>,
    mutations: Vec<Mutation>,
) -> Result<(), AppError> {
    if mutations.is_empty() {
        return Ok(());
    }
    txn.lock().await.mutate(mutations);
    Ok(())
}

/// `key -> value` の書き込みを表す [`Mutation`]。
pub(super) fn put_mutation(key: Vec<u8>, value: Vec<u8>) -> Mutation {
    Mutation {
        op: Op::Put as i32,
        key,
        value,
        assertion: Assertion::None as i32,
    }
}

/// `key` の削除を表す [`Mutation`]。
pub(super) fn delete_mutation(key: Vec<u8>) -> Mutation {
    Mutation {
        op: Op::Del as i32,
        key,
        value: Vec::new(),
        assertion: Assertion::None as i32,
    }
}

/// 削除と書き込みをまとめて 1 回の変更として適用する。
///
/// キー順に並べてから渡すので、ロック取得順に依存するデッドロック回避の前提を保てる
/// （削除と書き込みを別々に流すと、その間で順序が崩れる）。
pub(super) async fn write_batch(
    txn: &Readers<LazyTxn>,
    deletes: impl IntoIterator<Item = Vec<u8>>,
    puts: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
) -> Result<(), AppError> {
    let mut mutations: Vec<Mutation> = deletes
        .into_iter()
        .map(delete_mutation)
        .chain(
            puts.into_iter()
                .map(|(key, value)| put_mutation(key, value)),
        )
        .collect();
    mutations.sort_unstable_by(|a, b| a.key.cmp(&b.key));
    mutate_many(txn, mutations).await
}

pub(super) async fn put_many(
    txn: &Readers<LazyTxn>,
    entries: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
) -> Result<(), AppError> {
    let mutations = entries
        .into_iter()
        .map(|(key, value)| put_mutation(key, value))
        .collect();
    mutate_many(txn, mutations).await
}

pub(super) async fn delete_many(
    txn: &Readers<LazyTxn>,
    keys: impl IntoIterator<Item = Vec<u8>>,
) -> Result<(), AppError> {
    let mutations = keys.into_iter().map(delete_mutation).collect();
    mutate_many(txn, mutations).await
}

/// 複数キーをまとめて引く。存在しないキーは結果に現れない。
///
/// [`BATCH_KEYS`] で区切った各チャンクは互いに独立なので、追加スナップショットを
/// 開ける読み取り元（[`Readers::fanned_out`]）では**並行に**投げる。1 つの実体しか
/// 持たない読み取り元（書き込みトランザクション上の読み取り）では逐次に落ちる。
pub(super) async fn batch_get<R: Reader>(
    txn: &Readers<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    // 1 チャンクで収まるなら、スナップショットを増やす意味がない。
    if keys.len() > BATCH_KEYS
        && let Some(fanout) = &txn.fanout
    {
        return batch_get_fanned_out(fanout, keys).await;
    }

    let mut txn = txn.lock().await;
    // 1 塊で収まる通常の場合はそのまま渡す（`chunks(..).to_vec()` だと、収まる場合まで
    // キー列を丸ごと複製することになる）。
    if keys.len() <= BATCH_KEYS {
        return txn.read_many(keys).await.map_err(AppError::from);
    }

    let mut rest = keys;
    let mut out = Vec::new();
    while !rest.is_empty() {
        let tail = rest.split_off(rest.len().min(BATCH_KEYS));
        out.extend(txn.read_many(rest).await?);
        rest = tail;
    }
    Ok(out)
}

/// チャンクごとに独立したスナップショットを開き、[`MAX_FANOUT`] 本まで並行に読む。
///
/// どのスナップショットも同じタイムスタンプを読むので、どのチャンクがどれに
/// 割り当たっても結果は変わらない。
///
/// # 失敗しても飛んでいる RPC を打ち切らない
///
/// [`JoinSet`](tokio::task::JoinSet) は**drop すると中のタスクを abort する**。1 本の
/// チャンクが失敗した時点で `?` を返すと、まだ応答待ちの兄弟（最大 [`MAX_FANOUT`] - 1 本）が
/// その場で捨てられ、gRPC ストリームが RST で打ち切られる。TiKV 側にはそれが
/// `RemoteStopped`、こちら側には h2 の `locally-reset streams reached limit` として現れる。
///
/// 厄介なのは、これが**混んでいるときほど増える**こと。失敗 1 回につき数本を巻き添えに
/// するので、詰まりかけたクラスタへこちらからキャンセルを浴びせて悪化させる。
/// そこで失敗は覚えておくだけにして、飛んでいるぶんは最後まで見届けてから返す。
async fn batch_get_fanned_out(
    fanout: &Fanout,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    type ChunkResult = Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error>;

    let mut chunks = keys.chunks(BATCH_KEYS).map(<[Vec<u8>]>::to_vec);
    let mut running: tokio::task::JoinSet<ChunkResult> = tokio::task::JoinSet::new();
    let mut out = Vec::new();
    let mut failure: Option<AppError> = None;

    loop {
        // 失敗が判ったら新しいチャンクは投げない。ただし投げ終えたぶんは待つ。
        while failure.is_none()
            && running.len() < MAX_FANOUT
            && let Some(chunk) = chunks.next()
        {
            let mut snapshot = fanout.open();
            running.spawn(async move {
                Ok(snapshot
                    .batch_get(chunk)
                    .await?
                    .map(|kv| (Vec::from(kv.0), kv.1))
                    .collect())
            });
        }

        let Some(joined) = running.join_next().await else {
            break;
        };

        match joined
            .map_err(|e| AppError::InternalError(format!("batch get task: {e}")))
            .and_then(|result| result.map_err(AppError::from))
        {
            // 既に失敗が確定しているなら、読めた分は捨てる（返すのはエラーなので）。
            Ok(pairs) if failure.is_none() => out.extend(pairs),
            Ok(_) => {}
            Err(e) => {
                failure.get_or_insert(e);
            }
        }
    }

    match failure {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

fn range_from(start: Vec<u8>, end: Option<Vec<u8>>) -> BoundRange {
    match end {
        Some(end) => BoundRange::from(start..end),
        None => BoundRange::from(start..),
    }
}

/// プレフィックスに一致する全キーと値を取り出す（バッチ分割して読み切る）。
pub(super) async fn scan_prefix<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    let end = keys::prefix_end(prefix);
    let mut start = prefix.to_vec();
    let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    loop {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range(range_from(start.clone(), end.clone()), SCAN_BATCH)
                .await?
        };

        // 終端の判定は**生の取得件数**で行う。重ねるのは読み切ったあと。
        let fetched = batch.len();
        out.extend(batch);
        if fetched < SCAN_BATCH as usize {
            break;
        }
        // 次のバッチは最後に読んだキーの直後から。
        start = out.last().expect("バッチが空でない").0.clone();
        start.push(0);
    }

    let staged = txn.lock().await.staged_range(prefix, end.as_deref());
    if staged.is_empty() {
        return Ok(out);
    }
    // 未送信の変更を重ねる。キー順で返す約束は保つ。
    let mut merged: BTreeMap<Vec<u8>, Vec<u8>> = out.into_iter().collect();
    for (key, value) in staged {
        match value {
            Some(value) => merged.insert(key, value),
            None => merged.remove(&key),
        };
    }
    Ok(merged.into_iter().collect())
}

/// 指定範囲のキーだけを取り出す（バッチ分割して読み切る）。
/// `start`（含む）から `end`（排他、`None` なら終端まで）のキーを取り出す。
pub(super) async fn scan_keys_range<R: Reader>(
    txn: &Readers<R>,
    start: Vec<u8>,
    end: Option<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, AppError> {
    let mut cursor = start.clone();
    let mut out: Vec<Vec<u8>> = Vec::new();

    loop {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range_keys(range_from(cursor.clone(), end.clone()), SCAN_BATCH)
                .await?
        };

        // 終端の判定は**生の取得件数**で行う。重ねるのは読み切ったあと。
        let fetched = batch.len();
        out.extend(batch);
        if fetched < SCAN_BATCH as usize {
            break;
        }
        // 次のバッチは最後に読んだキーの直後から。
        cursor = out.last().expect("バッチが空でない").clone();
        cursor.push(0);
    }

    let staged = txn.lock().await.staged_range(&start, end.as_deref());
    if staged.is_empty() {
        return Ok(out);
    }
    // 未送信の変更を重ねる。キー順で返す約束は保つ。
    let mut merged: BTreeSet<Vec<u8>> = out.into_iter().collect();
    for (key, value) in staged {
        if value.is_some() {
            merged.insert(key);
        } else {
            merged.remove(&key);
        }
    }
    Ok(merged.into_iter().collect())
}

/// プレフィックスに一致する全キーだけを取り出す。
pub(super) async fn scan_prefix_keys<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<Vec<Vec<u8>>, AppError> {
    scan_keys_range(txn, prefix.to_vec(), keys::prefix_end(prefix)).await
}

/// `start`（含む）から `end_inclusive`（含む）までのキーを取り出す。
pub(super) async fn scan_inclusive_keys<R: Reader>(
    txn: &Readers<R>,
    start: Vec<u8>,
    end_inclusive: Vec<u8>,
) -> Result<Vec<Vec<u8>>, AppError> {
    // 終端を含めたいので、末尾に 0 を足して排他終端へ変換する。
    let mut end = end_inclusive;
    end.push(0);
    scan_keys_range(txn, start, Some(end)).await
}

// --- シャード値のフレーム ---
//
// シャードのペイロードは `SpatialIdMap` の rkyv バイト列で、読み出しは
// `access_unchecked`（構造を検証しないゼロコピーアクセス）を通る。その安全条件は
// 「自分が書いたバイト列そのものであること」で、ローカルの mmap ならファイル権限が
// それを担保していた。TiKV ではバイト列がネットワーク越しに届き、クラスタは
// 複数インスタンス・複数バージョンで共有されうるため、前提が成り立たない。
//
// そこで TiKV に保存するシャード値だけを CRC32 で包み、`unsafe` へ渡す前に検証する。
// 検証はペイロードを 1 度なめるだけで、コピーは発生しない（[`ShardValue::entry`] は
// 受信バッファへの借用を返す）ので、ゼロコピーはそのまま保たれる。
//
// LMDB 側は共通形式（`encoding::shard_entry`）のまま。両バックエンドで同じなのは
// **ペイロードの形式**であり、この枠はその外側の TiKV 固有の保存表現にあたる。

/// フレームのヘッダ長 = CRC32（u32 LE, 4）。
const FRAME_HEADER_LEN: usize = 4;

/// CRC 検証を通ったシャード値。
///
/// 受信したバッファをそのまま保持し、[`entry`](Self::entry) が検証済みペイロードへの
/// 借用を返す。ペイロードを別バッファへ写さないので、この先の rkyv アクセスは
/// 受信バッファを直接読む。
pub(super) struct ShardValue {
    framed: Vec<u8>,
}

impl ShardValue {
    /// 保存されていたバイト列を検証して受け取る。
    ///
    /// CRC が合わない場合は「壊れている」ものとして拒否する。ここを通過したバイト列だけが
    /// `access_unchecked` へ渡る。
    fn verify(framed: Vec<u8>) -> Result<Self, AppError> {
        if framed.len() < FRAME_HEADER_LEN {
            return Err(AppError::StorageError(
                "shard value is shorter than its frame header".to_string(),
            ));
        }
        let expected = u32::from_le_bytes(
            framed[..FRAME_HEADER_LEN]
                .try_into()
                .expect("FRAME_HEADER_LEN バイトある"),
        );
        let actual = crc32fast::hash(&framed[FRAME_HEADER_LEN..]);
        if actual != expected {
            return Err(AppError::StorageError(format!(
                "shard value failed its integrity check (crc32 expected {expected:#010x}, found {actual:#010x})"
            )));
        }
        Ok(Self { framed })
    }

    /// 検証済みのシャードエントリ（`encoding::shard_entry` の形式）。
    pub(super) fn entry(&self) -> &[u8] {
        &self.framed[FRAME_HEADER_LEN..]
    }
}

/// シャードエントリを保存用のフレームへ包む。
fn frame(entry: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + entry.len());
    out.extend_from_slice(&crc32fast::hash(entry).to_le_bytes());
    out.extend_from_slice(entry);
    out
}

// --- シャードの保存 ---
//
// シャードは 2 本のキーで表される。
//
// ```text
//   0x06 ‖ table_id ‖ flex_id -> crc32 ‖ ShardEntry     （本体）
//   0x08 ‖ table_id ‖ flex_id -> 保持件数（u32 LE）      （リーフのみ）
// ```
//
// 件数はエントリのヘッダにも入っているが、そちらを読むにはリーフの本体
// （`SpatialIdMap` の rkyv バイト列）ごと転送することになる。`table_count` は
// 件数を合算するだけなので、テーブル全体を転送するのは割に合わない。件数だけを
// 別キーへ切り出すと、集計はシャード数 × 4 バイトで済む。
//
// LMDB 側にこのキーは無い。あちらは mmap 上をストリームで舐められるので、
// 本体を持ってくる代償が無いため。`keys.rs` はもともとバックエンド固有なので、
// この差はレイアウトの差として閉じている。
//
// 2 本のキーが食い違わないよう、書き込みと削除は必ずこの節の関数を通す。

/// シャードの保存を表す変更を組み立てる。**ネットワークには触れない。**
///
/// 本体と件数の 2 本を必ず一組で返すのが要点で、片方だけ書いて食い違わせないための入口。
///
/// 送信と分けてあるのは、書き込み経路の重い計算（rkyv の復元・直列化）を非同期ワーカーの
/// 外へ出すため。変更を値として組み立てられれば、その計算ごと blocking タスクへ移せる
/// （`data.rs` の書き込みの節を参照）。
pub(super) fn shard_mutations(
    table_id: TableId,
    region: &FlexId,
    entry: &[u8],
) -> Result<[Mutation; 2], AppError> {
    let count_key = keys::shard_count(table_id, region);
    let count = match ShardEntry::leaf_count(entry)? {
        Some(count) => put_mutation(count_key, count.to_le_bytes().to_vec()),
        // ポインタノードは件数を持たない。リーフから昇格した場合に備えて消す。
        None => delete_mutation(count_key),
    };
    Ok([
        put_mutation(keys::shard(table_id, region), frame(entry)),
        count,
    ])
}

/// シャードの削除を表す変更（本体と件数の両方）。
pub(super) fn shard_deletions(table_id: TableId, region: &FlexId) -> [Mutation; 2] {
    [
        delete_mutation(keys::shard(table_id, region)),
        delete_mutation(keys::shard_count(table_id, region)),
    ]
}

/// 値インデックスの差分を表す変更を組み立てる。
///
/// 変更前後で同じキーは触らない（消してから書き直すと MVCC のバージョンが無駄に増える）。
pub(super) fn value_index_mutations(
    old_keys: &FxHashSet<Vec<u8>>,
    new_keys: &FxHashSet<Vec<u8>>,
    out: &mut Vec<Mutation>,
) {
    out.extend(old_keys.difference(new_keys).cloned().map(delete_mutation));
    out.extend(
        new_keys
            .difference(old_keys)
            .map(|key| put_mutation(key.clone(), Vec::new())),
    );
}

/// シャード領域をまとめてロックし、**ロック時点の**内容を返す。
///
/// 返る地図には要求した全領域が入る（未作成領域は `None`）。存在しないキーも
/// ロックされるので、「空の領域を他者が埋める」「親へ畳んで消す」といった操作も
/// この呼び出しで排他される。
///
/// キーは [`BTreeMap`] に集めてから渡すので、反復順（＝バイト昇順）がそのまま
/// デッドロック回避の全順序になる。
///
/// 読み取りは [`LazyTxn::lock_and_read`] を通るので、**このトランザクションで
/// 既に書いた内容が重なった状態**で返る。
pub(super) async fn lock_shards(
    txn: &Readers<LazyTxn>,
    table_id: TableId,
    regions: impl IntoIterator<Item = FlexId>,
) -> Result<BTreeMap<FlexId, Option<ShardValue>>, AppError> {
    // 領域 → キーの対応を作り、ロックは昇順のキー列で要求する。
    let by_key: BTreeMap<Vec<u8>, FlexId> = regions
        .into_iter()
        .map(|r| (keys::shard(table_id, &r), r))
        .collect();
    if by_key.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut found = {
        let mut txn = txn.lock().await;
        txn.lock_and_read(by_key.keys().cloned().collect()).await?
    };

    // 存在したものを埋め、残りは未作成として `None` にする。
    let mut out = BTreeMap::new();
    for (key, region) in by_key {
        let value = match found.remove(&key) {
            Some(framed) => Some(ShardValue::verify(framed)?),
            None => None,
        };
        out.insert(region, value);
    }
    Ok(out)
}

/// プレフィックスに一致するキーを、上限件数まで削除する。
///
/// 1 トランザクションに全部を詰めると TiKV のトランザクションサイズ上限に当たるので、
/// 呼び出し側が繰り返し呼べるよう「削除した件数」を返す。0 なら消し切っている。
pub(super) async fn delete_prefix_chunk(
    txn: &Readers<LazyTxn>,
    prefix: &[u8],
    limit: u32,
) -> Result<usize, AppError> {
    let end = keys::prefix_end(prefix);
    let batch = {
        let mut txn = txn.lock().await;
        let raw = txn
            .read_range_keys(range_from(prefix.to_vec(), end.clone()), limit)
            .await?;
        // 同じトランザクションで既に消したキーは数えない。数えてしまうと
        // 「まだ残っている」と読めてしまい、呼び出し側の繰り返しが止まらなくなる。
        let staged = txn.staged_range(prefix, end.as_deref());
        if staged.is_empty() {
            raw
        } else {
            let deleted: BTreeSet<&Vec<u8>> = staged
                .iter()
                .filter(|(_, value)| value.is_none())
                .map(|(key, _)| key)
                .collect();
            raw.into_iter()
                .filter(|key| !deleted.contains(key))
                .collect()
        }
    };
    let removed = batch.len();
    delete_many(txn, batch).await?;
    Ok(removed)
}

/// テーブルが保持する [`FlexId`] の総数を、件数キーだけを読んで合算する。
pub(super) async fn table_flex_id_count<R: Reader>(
    txn: &Readers<R>,
    table_id: TableId,
) -> Result<u64, AppError> {
    let entries = scan_prefix(txn, &keys::shard_counts_of(table_id)).await?;
    let mut total = 0u64;
    for (_, value) in entries {
        let bytes: [u8; 4] = value.as_slice().try_into().map_err(|_| {
            AppError::StorageError("shard count entry is not four bytes".to_string())
        })?;
        total += u32::from_le_bytes(bytes) as u64;
    }
    Ok(total)
}

pub(super) async fn batch_get_shards<R: Reader>(
    txn: &Readers<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    batch_get(txn, keys)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::verify(framed)?)))
        .collect()
}

pub(super) async fn scan_shard_prefix<R: Reader>(
    txn: &Readers<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    scan_prefix(txn, prefix)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::verify(framed)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_value_round_trips() {
        let entry = b"shard entry bytes".to_vec();
        let value = ShardValue::verify(frame(&entry)).unwrap();
        assert_eq!(value.entry(), entry.as_slice());
    }

    #[test]
    fn empty_entry_round_trips() {
        let value = ShardValue::verify(frame(&[])).unwrap();
        assert_eq!(value.entry(), b"");
    }

    #[test]
    fn corrupted_payload_is_rejected() {
        let mut framed = frame(b"shard entry bytes");
        // ペイロードの 1 バイトだけを壊す（CRC ヘッダは触らない）。
        let last = framed.len() - 1;
        framed[last] ^= 0xFF;
        assert!(ShardValue::verify(framed).is_err());
    }

    #[test]
    fn truncated_value_is_rejected() {
        let mut framed = frame(b"shard entry bytes");
        framed.truncate(framed.len() - 3);
        assert!(ShardValue::verify(framed).is_err());
        // ヘッダにも届かない長さも弾く（スライスで panic しないこと）。
        assert!(ShardValue::verify(vec![0x00, 0x01]).is_err());
        assert!(ShardValue::verify(Vec::new()).is_err());
    }
}
