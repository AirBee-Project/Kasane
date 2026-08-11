//! 読み取りトランザクション上で行える操作。

use kasane_logic::{FlexId, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{Table, TableDataType};
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::User;

use super::{CatalogRepository, ValueGroups};

/// 名前から解決した `(データベース, テーブル)`。
///
/// **どちらも「無い」ことをエラーにしない**のが要点。認可の判定は名前の解決結果を
/// 必要とするが、判定より先に「存在しない」を返してしまうと、権限の無い利用者へ
/// 名前の存在有無を教えることになる。呼び出し側が認可 → 存在確認の順に処理できるよう、
/// ここでは解決できたかどうかだけを返す。
#[derive(Debug, Clone)]
pub struct ResolvedTable {
    pub db_id: Option<DatabaseId>,
    pub table: Option<Table>,
}

/// 読み取りトランザクション上で行える操作。
// `async fn` の戻り値の Future には呼び出し側から `Send` 境界を付けられない。
// このアプリではバックエンドが feature で 1 つに確定し、Send 性は具体型経由で
// 漏れ出すため、trait 側で境界を要求する必要がない（`storage.rs` の設計メモを参照）。
// 署名の読みやすさを優先して `async fn` を使う。
#[allow(async_fn_in_trait)]
pub trait ReadRepository: CatalogRepository {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError>;

    /// Database の一覧を [`DatabaseId`] つきで取得する。
    /// 呼び出し側は権限の絞り込みに ID を使うため、引き直さずに済むよう ID を添えて返す。
    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError>;

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError>;

    /// `(データベース名, テーブル名)` の並びを**まとめて**解決する。返りは入力と同じ並び。
    ///
    /// 認可とサービス処理は同じキーを必要とする（前者は `(db_id, table_id)`、後者は
    /// テーブルのメタデータ）。別々に引くと同じキーを 2 度読むことになり、しかもその
    /// 2 度は別のトランザクションになるので、往復が丸ごと 1 組ぶん増える。
    ///
    /// 名前の解決はバックエンドによっては 1 件ずつがネットワーク往復なので、
    /// **複数件を 1 度に渡せる形**にしてある（TiKV 実装はデータベースとテーブルを
    /// それぞれ 1 回の `batch_get` にまとめる）。
    async fn resolve_tables(
        &self,
        refs: &[(String, String)],
    ) -> Result<Vec<ResolvedTable>, AppError>;

    async fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError>;

    /// 既に ID を解決済みの呼び出し側が、名前からの引き直しを避けるために使う。
    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError>;

    /// テーブルが保持する [`FlexId`] の総数を返す。
    async fn table_count(&self, table_id: TableId) -> Result<u64, AppError>;

    /// 指定された範囲の空間 ID を値ごとにグループ化して返す。
    async fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError>;

    /// 値が `value` と等しい FlexId を引く（値インデックス経由）。
    async fn data_filter_eq(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError>;

    /// 値が `lo`〜`hi`（両端含む）に入る FlexId を引く。
    async fn data_filter_range(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError>;

    async fn get_user(&self, username: &str) -> Result<Option<User>, AppError>;

    async fn require_user(&self, username: &str) -> Result<User, AppError>;

    async fn get_all_users(&self) -> Result<Vec<User>, AppError>;
}
