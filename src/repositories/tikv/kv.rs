//! トランザクション上の素の KV 操作と、シャード値のフレーム。
//!
//! 読み取りは [`TikvRead`](super::TikvRead) と [`TikvWrite`](super::TikvWrite) の
//! どちらからも必要なので、トランザクションだけを受け取る自由関数として置いている。
//!
//! # なぜ読み取りが [`Reader`] で抽象化されているか
//!
//! 通常の読み書きは [`tikv_client::Transaction`] 上で行うが、クエリ実行器の入力源
//! （`query_source.rs`）は**開始タイムスタンプを固定した**読み取りを何度も行う必要があり、
//! そちらは [`tikv_client::Snapshot`] を使う。両者は読み取り系のシグネチャが同一なので、
//! ここで 1 つの trait にまとめ、下の自由関数を両方で使い回す。

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use kasane_logic::FlexId;
use tikv_client::proto::kvrpcpb::{Assertion, Mutation, Op};
use tikv_client::{BoundRange, Snapshot, Transaction, TransactionClient};
use tokio::sync::Mutex;

use super::keys;
use crate::error::AppError;
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::ShardEntry;

use super::to_app_error;

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
    /// このトランザクションで書いた値（`None` は削除）。
    ///
    /// [`lock_and_read`](Self::lock_and_read) が使う。詳細はそちらの注記を参照。
    /// 保持するのは**キーごとに最新の 1 つ**なので、書き込み回数ではなく
    /// 触ったキー数に比例する。
    written: HashMap<Vec<u8>, Option<Vec<u8>>>,
    /// 競合由来の失敗を受け取ったか。
    ///
    /// ロックの取得はこのトランザクションの**内側**で起きるので、競合は
    /// `Storage::write` から見ると「クロージャが返したアプリケーションエラー」に
    /// 見えてしまう。それではやり直しの判断ができないため、ここに記録して
    /// `Storage::write` が拾えるようにする。
    conflicted: bool,
}

impl LazyTxn {
    pub(super) fn new(client: Arc<TransactionClient>) -> Self {
        Self {
            client,
            txn: None,
            written: HashMap::new(),
            conflicted: false,
        }
    }

    /// tikv のエラーを記録しつつそのまま返す。
    ///
    /// 競合由来なら印を付けておき、`Storage::write` に新しいトランザクションで
    /// やり直させる。**このトランザクションが出すエラーは必ずここを通る**
    /// （下のメソッドがすべて自分で通す）ので、呼び出し側が記録し忘れる余地がない。
    fn note(&mut self, err: tikv_client::Error) -> tikv_client::Error {
        if super::is_retryable(&err) {
            self.conflicted = true;
        }
        err
    }

    /// このトランザクションで書いたキーを記録する。
    ///
    /// 記録するのはシャード本体だけでよい。この台帳を読むのは
    /// [`lock_and_read`](Self::lock_and_read) だけで、そこへ渡るのは
    /// [`lock_shards`] が組み立てたシャードキーに限られるため。値インデックスや
    /// カタログまで抱えると、テーブル複製や回収のたびにテーブル 1 個分の
    /// バイト列を二重に持つことになる。
    fn record(&mut self, key: &[u8], value: Option<&[u8]>) {
        if key.first() != Some(&(keys::Ns::TablesData as u8)) {
            return;
        }
        self.written.insert(key.to_vec(), value.map(<[u8]>::to_vec));
    }

    /// 競合でやり直せる失敗を受け取ったか。
    pub(super) fn conflicted(&self) -> bool {
        self.conflicted
    }

    async fn open(&mut self) -> Result<&mut Transaction, tikv_client::Error> {
        if self.txn.is_none() {
            self.txn = Some(self.client.begin_pessimistic().await?);
        }
        Ok(self.txn.as_mut().expect("直前に開いた"))
    }

    /// 開いていたトランザクションを取り出す。一度も触れていなければ `None`。
    pub(super) fn into_opened(self) -> Option<Transaction> {
        self.txn
    }

    async fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), tikv_client::Error> {
        self.record(&key, Some(&value));
        let result = self.open().await?.put(key, value).await;
        result.map_err(|e| self.note(e))
    }

    async fn delete(&mut self, key: Vec<u8>) -> Result<(), tikv_client::Error> {
        self.record(&key, None);
        let result = self.open().await?.delete(key).await;
        result.map_err(|e| self.note(e))
    }

    async fn mutate(&mut self, mut mutations: Vec<Mutation>) -> Result<(), tikv_client::Error> {
        for m in &mutations {
            let value = (m.op != Op::Del as i32).then_some(m.value.as_slice());
            self.record(&m.key, value);
        }
        // 1 リクエストへ載せすぎないよう区切る（`BATCH_KEYS` の注記を参照）。
        // 手持ちの `Vec` を切り出して渡すので、1 塊で収まる通常の場合は複製が起きない。
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
    /// `batch_get_for_update` はトランザクションのローカルバッファを**見ない**
    /// （`get` は見るが、こちらはロック応答の値をそのまま返す）。そのため、同じ
    /// トランザクションで既に書いたキーを引き直すと、コミット前の自分の変更が
    /// 消えて見える。1 回の書き込みクロージャで同じリーフを 2 度触る操作
    /// （まとめて `data_insert` する等）が自分の書き込みを失うので、
    /// ここで手元の記録を優先させる。
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
            match self.written.get(&key) {
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

impl Reader for LazyTxn {
    async fn read_one(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, tikv_client::Error> {
        self.open().await?.read_one(key).await
    }

    async fn read_many(
        &mut self,
        keys: Vec<Vec<u8>>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        self.open().await?.read_many(keys).await
    }

    async fn read_range(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, tikv_client::Error> {
        self.open().await?.read_range(range, limit).await
    }

    async fn read_range_keys(
        &mut self,
        range: BoundRange,
        limit: u32,
    ) -> Result<Vec<Vec<u8>>, tikv_client::Error> {
        self.open().await?.read_range_keys(range, limit).await
    }
}

// --- 素の KV 操作 ---

pub(super) async fn get<R: Reader>(
    txn: &Mutex<R>,
    key: Vec<u8>,
) -> Result<Option<Vec<u8>>, AppError> {
    let mut txn = txn.lock().await;
    txn.read_one(key).await.map_err(to_app_error)
}

pub(super) async fn put(
    txn: &Mutex<LazyTxn>,
    key: Vec<u8>,
    value: Vec<u8>,
) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    txn.put(key, value).await.map_err(to_app_error)
}

pub(super) async fn delete(txn: &Mutex<LazyTxn>, key: Vec<u8>) -> Result<(), AppError> {
    let mut txn = txn.lock().await;
    txn.delete(key).await.map_err(to_app_error)
}

/// 複数キーへの変更をまとめて適用する。
///
/// 悲観トランザクションの `put` / `delete` は**キーごとに**悲観ロックを取るため、
/// 1 件につき TSO 取得 1 回と PessimisticLock RPC 1 回がかかる。`batch_mutate` は
/// 全キーのロックを 1 リクエストへ束ねる（リージョンごとに分割される）ので、
/// 差分が大きいほど往復数の差が効く。
pub(super) async fn mutate_many(
    txn: &Mutex<LazyTxn>,
    mutations: Vec<Mutation>,
) -> Result<(), AppError> {
    if mutations.is_empty() {
        return Ok(());
    }
    let mut txn = txn.lock().await;
    txn.mutate(mutations).await.map_err(to_app_error)
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
    txn: &Mutex<LazyTxn>,
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
    txn: &Mutex<LazyTxn>,
    entries: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
) -> Result<(), AppError> {
    let mutations = entries
        .into_iter()
        .map(|(key, value)| put_mutation(key, value))
        .collect();
    mutate_many(txn, mutations).await
}

pub(super) async fn delete_many(
    txn: &Mutex<LazyTxn>,
    keys: impl IntoIterator<Item = Vec<u8>>,
) -> Result<(), AppError> {
    let mutations = keys.into_iter().map(delete_mutation).collect();
    mutate_many(txn, mutations).await
}

/// 複数キーをまとめて引く。存在しないキーは結果に現れない。
pub(super) async fn batch_get<R: Reader>(
    txn: &Mutex<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut txn = txn.lock().await;
    let mut out = Vec::new();
    for chunk in keys.chunks(BATCH_KEYS) {
        out.extend(txn.read_many(chunk.to_vec()).await.map_err(to_app_error)?);
    }
    Ok(out)
}

fn range_from(start: Vec<u8>, end: Option<Vec<u8>>) -> BoundRange {
    match end {
        Some(end) => BoundRange::from(start..end),
        None => BoundRange::from(start..),
    }
}

/// プレフィックスに一致する全キーと値を取り出す（バッチ分割して読み切る）。
pub(super) async fn scan_prefix<R: Reader>(
    txn: &Mutex<R>,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, AppError> {
    let end = keys::prefix_end(prefix);
    let mut start = prefix.to_vec();
    let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    loop {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range(range_from(start.clone(), end.clone()), SCAN_BATCH)
                .await
                .map_err(to_app_error)?
        };

        let fetched = batch.len();
        out.extend(batch);
        if fetched < SCAN_BATCH as usize {
            break;
        }
        // 次のバッチは最後に読んだキーの直後から。
        start = out.last().expect("バッチが空でない").0.clone();
        start.push(0);
    }

    Ok(out)
}

/// 指定範囲のキーだけを取り出す（バッチ分割して読み切る）。
/// `start`（含む）から `end`（排他、`None` なら終端まで）のキーを取り出す。
pub(super) async fn scan_keys_range<R: Reader>(
    txn: &Mutex<R>,
    start: Vec<u8>,
    end: Option<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, AppError> {
    let mut cursor = start;
    let mut out: Vec<Vec<u8>> = Vec::new();

    loop {
        let batch = {
            let mut txn = txn.lock().await;
            txn.read_range_keys(range_from(cursor.clone(), end.clone()), SCAN_BATCH)
                .await
                .map_err(to_app_error)?
        };

        let fetched = batch.len();
        out.extend(batch);
        if fetched < SCAN_BATCH as usize {
            break;
        }
        // 次のバッチは最後に読んだキーの直後から。
        cursor = out.last().expect("バッチが空でない").clone();
        cursor.push(0);
    }

    Ok(out)
}

/// プレフィックスに一致する全キーだけを取り出す。
pub(super) async fn scan_prefix_keys<R: Reader>(
    txn: &Mutex<R>,
    prefix: &[u8],
) -> Result<Vec<Vec<u8>>, AppError> {
    scan_keys_range(txn, prefix.to_vec(), keys::prefix_end(prefix)).await
}

/// `start`（含む）から `end_inclusive`（含む）までのキーを取り出す。
pub(super) async fn scan_inclusive_keys<R: Reader>(
    txn: &Mutex<R>,
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

/// シャードエントリを保存する。リーフなら件数キーも同時に更新する。
pub(super) async fn put_shard(
    txn: &Mutex<LazyTxn>,
    table_id: TableId,
    region: &FlexId,
    entry: &[u8],
) -> Result<(), AppError> {
    let count_key = keys::shard_count(table_id, region);
    let count = match ShardEntry::leaf_count(entry)? {
        Some(count) => put_mutation(count_key, count.to_le_bytes().to_vec()),
        // ポインタノードは件数を持たない。リーフから昇格した場合に備えて消す。
        None => delete_mutation(count_key),
    };
    // 本体と件数を 1 リクエストにまとめる。悲観トランザクションでは 1 キーごとに
    // タイムスタンプ取得とロック RPC がかかるので、分けると往復が倍になる。
    mutate_many(
        txn,
        vec![
            put_mutation(keys::shard(table_id, region), frame(entry)),
            count,
        ],
    )
    .await
}

/// シャードエントリを削除する（件数キーも一緒に消える）。
pub(super) async fn delete_shard(
    txn: &Mutex<LazyTxn>,
    table_id: TableId,
    region: &FlexId,
) -> Result<(), AppError> {
    delete_many(
        txn,
        [
            keys::shard(table_id, region),
            keys::shard_count(table_id, region),
        ],
    )
    .await
}

/// シャード領域をまとめてロックし、**ロック時点の**内容を返す。
///
/// 返る地図には要求した全領域が入る（未作成領域は `None`）。存在しないキーも
/// ロックされるので、「空の領域を他者が埋める」「親へ畳んで消す」といった操作も
/// この呼び出しで排他される。
///
/// キーは [`BTreeSet`](std::collections::BTreeSet) に集めてから渡すこと。反復順
/// （＝バイト昇順）がそのままデッドロック回避の全順序になる。
pub(super) async fn lock_shards(
    txn: &Mutex<LazyTxn>,
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
        txn.lock_and_read(by_key.keys().cloned().collect())
            .await
            .map_err(to_app_error)?
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
    txn: &Mutex<LazyTxn>,
    prefix: &[u8],
    limit: u32,
) -> Result<usize, AppError> {
    let end = keys::prefix_end(prefix);
    let batch = {
        let mut txn = txn.lock().await;
        txn.read_range_keys(range_from(prefix.to_vec(), end), limit)
            .await
            .map_err(to_app_error)?
    };
    let removed = batch.len();
    delete_many(txn, batch).await?;
    Ok(removed)
}

/// テーブルが保持する [`FlexId`] の総数を、件数キーだけを読んで合算する。
pub(super) async fn table_flex_id_count<R: Reader>(
    txn: &Mutex<R>,
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

pub(super) async fn get_shard<R: Reader>(
    txn: &Mutex<R>,
    key: Vec<u8>,
) -> Result<Option<ShardValue>, AppError> {
    match get(txn, key).await? {
        Some(framed) => Ok(Some(ShardValue::verify(framed)?)),
        None => Ok(None),
    }
}

pub(super) async fn batch_get_shards<R: Reader>(
    txn: &Mutex<R>,
    keys: Vec<Vec<u8>>,
) -> Result<Vec<(Vec<u8>, ShardValue)>, AppError> {
    batch_get(txn, keys)
        .await?
        .into_iter()
        .map(|(key, framed)| Ok((key, ShardValue::verify(framed)?)))
        .collect()
}

pub(super) async fn scan_shard_prefix<R: Reader>(
    txn: &Mutex<R>,
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
