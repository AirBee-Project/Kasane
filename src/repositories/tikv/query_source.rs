//! TiKV 上のシャードツリーを、Kasane-Logic のクエリ入力源として見せるアダプタ。
//!
//! # スナップショットを固定する理由
//!
//! [`Source::read_subset`] は 1 回のクエリ実行で**複数回**呼ばれる（対象領域ごと、
//! そしてクエリが同じテーブルを複数箇所で参照すればその数だけ）。呼ばれるたびに
//! 新しいトランザクションを開くと、そのつど別の `start_ts` で読むことになり、
//! 途中で走った書き込みを一部だけ見た結果が混ざりうる。
//!
//! そこでアダプタの構築時に**タイムスタンプを 1 つ確定させ**、以降の読み取りは
//! すべてその時点のスナップショットから行う。LMDB 側は単一ライタかつ読み取りが
//! ローカルなので実害が出にくいが、TiKV では複数インスタンスの同時書き込みが
//! 通常運用なので、ここを固定しないとクエリ結果が引き裂かれる。
//!
//! # ランタイムハンドルを持ち回る理由
//!
//! Kasane-Logic の [`Source`] は同期 I/F なので、非同期の TiKV アクセスをその内側で
//! 完了させる必要がある。[`tokio::runtime::Handle::current`] を `read_subset` の中で
//! 呼ぶと、実行器が別スレッドプール（rayon など）から読みに来たときにランタイム文脈が
//! 無くて panic する。構築はサービス層（ランタイム文脈内）で行われるので、
//! そこで取得したハンドルを持ち回れば、どのスレッドから呼ばれても成立する。

use kasane_logic::{Error as LogicError, RangeId, SafeValue, Source, WorkingTree};
use tikv_client::Timestamp;
use tokio::runtime::Handle;

use crate::models::id::TableId;
use crate::repositories::traits::DecodeFn;

use super::{TikvDb, TikvRead};

/// 1 テーブルを 1 つのクエリ入力源として見せるアダプタ。
///
/// トランザクションは保持しない（`Source` は実行器から複数回・複数スレッドで呼ばれうる）。
/// 代わりに**開始タイムスタンプ**を保持し、読み取りのたびにその時点のスナップショットを
/// 開く。こうすると同じクエリ内の全読み取りが同一の断面を見る。
pub struct TikvTableSource<V> {
    db: TikvDb,
    table_id: TableId,
    decode: DecodeFn<V>,
    /// このクエリが見る断面。構築時に 1 度だけ確定する。
    snapshot_ts: Timestamp,
    /// 構築時（ランタイム文脈内）に捕まえたハンドル。
    handle: Handle,
}

impl TikvDb {
    /// テーブル 1 つをクエリの入力源として見せるアダプタを作る。
    ///
    /// ストレージのハンドルをサービス層へ露出させないための入口
    /// （LMDB 側の同名メソッドと対になる）。
    ///
    /// 同一クエリ内の複数ソースが同じ断面を見るよう、`snapshot_ts` は
    /// [`TikvDb::query_snapshot`] で 1 度だけ取ったものを共有して渡す。
    pub fn table_source<V>(
        &self,
        table_id: TableId,
        decode: DecodeFn<V>,
        snapshot_ts: Timestamp,
    ) -> TikvTableSource<V> {
        TikvTableSource {
            db: self.clone(),
            table_id,
            decode,
            snapshot_ts,
            handle: Handle::current(),
        }
    }
}

impl<V> Source for TikvTableSource<V>
where
    V: SafeValue + 'static,
{
    type Value = V;

    fn read_subset(&self, bounds: &[RangeId]) -> Result<WorkingTree<V>, LogicError> {
        let db = self.db.clone();
        let table_id = self.table_id;
        let bounds = bounds.to_vec();
        let snapshot_ts = self.snapshot_ts.clone();
        let decode = self.decode.clone();

        let flex_ids = self.handle.block_on(async move {
            // 固定タイムスタンプのスナップショット。読み取り専用なので
            // commit も rollback も要らず、drop するだけで閉じられる。
            let reader = TikvRead::at(db.client.clone(), snapshot_ts);

            // 境界は 1 本ずつではなく**まとめて**渡す。実行器はここへ全境界を一度に
            // 寄こすので、1 本ごとに木を降りると往復が「境界の本数 × 木の深さ」になる。
            // 境界の本数は対象空間 ID の広さに比例して増えるので、そこが要求の大きさに
            // 対する直列な鎖になっていた。
            //
            // 復元関数も一緒に渡す。格納バイト列を `Vec<u8>` で受け取ってからここで
            // 復元すると、結果 1 行につきヒープ確保が 1 回起き、しかもその `Vec` は
            // 復元した直後に捨てられる。葉を走査しながらその場で復元すれば起きない。
            reader
                .read_values_in_ranges(table_id, &bounds, decode.as_ref())
                .await
                .map_err(|e| LogicError::SourceRead(e.to_string()))
        })?;

        // 重なり合う bounds から同じ FlexId を複数回読んでも、いずれも同じ
        // `(FlexId, 値)` なので union がそのまま吸収する。
        Ok(flex_ids.into_iter().collect())
    }

    fn read_all(self: Box<Self>) -> Result<WorkingTree<V>, LogicError> {
        // テーブル全体の materialize は容量的に現実的でないため提供しない。
        Err(LogicError::Unsupported(
            "full scan of a database-backed table; use a bounded (lazy) query instead",
        ))
    }
}
