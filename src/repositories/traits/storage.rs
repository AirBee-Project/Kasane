//! ストレージ本体とトランザクション境界。

use crate::error::AppError;

use super::{ReadRepository, WriteRepository};

/// ストレージ本体。トランザクション境界を提供する。
///
/// `Clone` はハンドルの複製で、データ本体は複製しない。
// バックエンドは feature で 1 つに確定し Send 性は具体型経由で漏れ出すので、要求しない。
#[allow(async_fn_in_trait)]
pub trait Storage: Clone + Send + Sync + 'static {
    type Read<'a>: ReadRepository
    where
        Self: 'a;
    type Write<'a>: WriteRepository
    where
        Self: 'a;

    /// クエリ 1 回分の読み取り断面。
    type QuerySnapshot: Clone + Send + 'static;

    /// このクエリが見る断面を確定させる。
    ///
    /// [`Source`](kasane_logic::Source) は対象領域ごと・参照箇所ごとに何度も読みに来るので、
    /// そのたびに断面を取り直すと途中で走った書き込みを一部だけ見た結果が混ざる。
    async fn query_snapshot(&self) -> Result<Self::QuerySnapshot, AppError>;

    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        // 'a と 'b を同一にすると、クロージャ実行後にトランザクションを閉じられなくなる。
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static;

    /// 成功したらコミットし、クロージャがエラーを返したら破棄する。
    ///
    /// **クロージャは複数回実行されうる**（競合やロック不足でのやり直し）ので、副作用を
    /// 外へ持ち出さないこと。[`AsyncFnMut`] ではなく [`AsyncFnOnce`] + [`Clone`] なのは、
    /// 再実行の必要性を呼び出し側の規律ではなく型で担保するため。
    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static;
}
