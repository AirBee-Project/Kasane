//! ストレージ本体。トランザクション境界を提供する。

use crate::error::AppError;

use super::{ReadRepository, WriteRepository};

/// ストレージ本体。トランザクション境界を提供する。
///
/// `Clone` を要求するのは `AppState` に持たせて各リクエストへ配るため
/// （実装はハンドルの複製であり、データ本体は複製しない）。
// `async fn` の戻り値の Future には呼び出し側から `Send` 境界を付けられない。
// このアプリではバックエンドが feature で 1 つに確定し、Send 性は具体型経由で
// 漏れ出すため、trait 側で境界を要求する必要がない（`storage.rs` の設計メモを参照）。
// 署名の読みやすさを優先して `async fn` を使う。
#[allow(async_fn_in_trait)]
pub trait Storage: Clone + Send + Sync + 'static {
    type Read<'a>: ReadRepository
    where
        Self: 'a;
    type Write<'a>: WriteRepository
    where
        Self: 'a;

    /// クエリ 1 回分の読み取り断面。
    ///
    /// 中身はバックエンドが決める（LMDB は読み取りトランザクション、TiKV は開始
    /// タイムスタンプ）。サービス層はこれを [`query_snapshot`](Self::query_snapshot) で
    /// 1 つ作り、そのクエリが使う全テーブルの入力源へ配るだけでよい。
    type QuerySnapshot: Clone + Send + 'static;

    /// このクエリが見る断面を確定させる。
    ///
    /// クエリ実行器の [`Source`](kasane_logic::Source) は対象領域ごと・参照箇所ごとに
    /// 何度も読みに来る。読むたびに断面を取り直すと、途中で走った書き込みを一部だけ
    /// 見た結果が混ざる。断面をここで 1 つに固定することで、1 回のクエリ結果が
    /// 必ず「ある一点のデータベースの状態」に対応する。
    async fn query_snapshot(&self) -> Result<Self::QuerySnapshot, AppError>;

    /// 読み取りトランザクションを開いてクロージャを実行する。
    ///
    /// 戻り値の Future に `Send` を課していないのは、バックエンドが feature で 1 つに
    /// 確定するため、具体型経由の呼び出しで auto-trait が漏れ出すのに任せる方が境界が
    /// 単純になるから（`AsyncFnOnce::CallOnceFuture` への HRTB 境界を書かずに済む）。
    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        // 参照のライフタイム 'a とハンドル自身のライフタイム 'b は分ける。同一にすると
        // 「ハンドルへの借用がハンドルの指すデータと同じだけ生きる」という過剰な制約になり、
        // クロージャ実行後にトランザクションを閉じられなくなる。
        // `Storage: 'static` があるので任意の 'b について `Self::Read<'b>` は成立する。
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static;

    /// 書き込みトランザクションを開いてクロージャを実行し、成功したらコミットする。
    /// クロージャがエラーを返した場合はコミットせず、トランザクションを破棄する。
    ///
    /// **クロージャは複数回実行されうる**（競合やロック不足でのやり直し）。そのため
    /// `Clone` を要求し、やり直しのたびに複製したものを使う。クロージャの外へ副作用を
    /// 持ち出さないこと。
    ///
    /// `AsyncFnMut` ではなく `AsyncFnOnce + Clone` にしているのは、再実行の必要性を
    /// 呼び出し側の規律ではなく型で担保するため。クロージャ本体は捕捉した値をそのまま
    /// 消費してよく、やり直しが起きないバックエンド（LMDB は単一ライタなので 1 回で確定）
    /// では複製そのものが発生しない。
    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static;
}
