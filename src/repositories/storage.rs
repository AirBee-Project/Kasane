//! ストレージバックエンドの抽象化。
//!
//! アプリケーションの状態はすべてこの層の向こう側に閉じ、上位（サービス層・ハンドラ層）は
//! ここで定義した trait だけを通してデータへ触れる。LMDB / TiKV の差し替えは Cargo feature
//! による排他選択で行い、選ばれた実装がこれらの trait を満たす。
//!
//! # トランザクション境界の形
//!
//! [`Storage::read`] / [`Storage::write`] はクロージャを受け取る。ハンドルを返して
//! 呼び出し側に手続き的に操作させる形にしていないのは、**書き込みがやり直しになりうる**ため。
//! TiKV では悲観ロックの取得が `PessimisticRetry` で弾かれることがあり、その場合は
//! 操作全体を新しいトランザクションで再実行する必要がある。クロージャを受け取る形なら
//! リトライをこの層の内部に閉じ込められ、サービス層は競合の存在を知らずに済む。
//! （結果として「書き込みは待たされても失敗しない」という LMDB の性質が保たれる。）
//!
//! そのため書き込みクロージャは **複数回呼ばれうる**（[`AsyncFnMut`]）。副作用を
//! クロージャの外へ持ち出さないこと。
//!
//! 詳細な実測の経緯は `docs/tikv-migration-phase0.md` を参照。

use std::future::Future;

use kasane_logic::{FlexId, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, UpdateTableConstraints,
};
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::{
    MAX_PRIVILEGE_RULES, PrivilegeRule, PrivilegeTarget, StoredPrivilege, StoredTarget, User,
    UserMetadata,
};

/// `data_get` の戻り値：`(値バイト, その値を持つ FlexId 群)` の一覧。
pub type ValueGroups = Vec<(Vec<u8>, Vec<FlexId>)>;

/// 格納バイト列を、クエリで扱う値型へ復元する関数。
///
/// テーブルの `data_type` に応じてサービス層が渡す（例: Int なら 8 バイト BE → `i64`）。
/// `Enum` の ID→文字列の逆引き表など、テーブルごとの前計算を閉じ込めるため
/// 関数ポインタではなくクロージャを取る。
///
/// `None` を返したセルは**結果から除外される**（型に合わない格納値）。
pub type DecodeFn<V> = std::sync::Arc<dyn Fn(&[u8]) -> Option<V> + Send + Sync>;

/// メタデータの点参照と、その上に載る権限ルールの名前 ⇄ ID 変換。
///
/// 読み取り・書き込みどちらのトランザクションからも必要になるため独立した trait にしている。
/// 実装が用意するのは点参照だけで、変換ロジックは既定実装として 1 箇所に置く。
pub trait MetaRepository {
    // --- 実装が用意する点参照 ---

    fn database_id(&self, name: &str)
    -> impl Future<Output = Result<Option<DatabaseId>, AppError>>;

    fn table_id(
        &self,
        db_id: DatabaseId,
        table_name: &str,
    ) -> impl Future<Output = Result<Option<TableId>, AppError>>;

    /// `DatabaseId` からデータベース名を引く。
    fn database_name(
        &self,
        db_id: DatabaseId,
    ) -> impl Future<Output = Result<Option<String>, AppError>>;

    /// `TableId` からテーブル名を引く。
    fn table_name(
        &self,
        table_id: TableId,
    ) -> impl Future<Output = Result<Option<String>, AppError>>;

    fn user_meta(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<Option<UserMetadata>, AppError>>;

    /// データベース配下のテーブル名を列挙する。
    fn table_names(&self, db_id: DatabaseId)
    -> impl Future<Output = Result<Vec<String>, AppError>>;

    // --- 上に載る共通ロジック ---

    fn require_database_id(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<DatabaseId, AppError>> {
        async move {
            self.database_id(name)
                .await?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: name.to_string(),
                })
        }
    }

    fn require_user_meta(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<UserMetadata, AppError>> {
        async move {
            self.user_meta(username)
                .await?
                .ok_or_else(|| AppError::NotFound("User not found".into()))
        }
    }

    /// API 表現（名前ベース）の権限ルール列を保存形式（ID ベース）へ解決する。
    ///
    /// - 存在しないデータベース／テーブルを指すルールは 404 で拒否する
    /// - 同じ対象を指すルールが複数あればロールが一致する場合のみ 1 件に畳み、
    ///   食い違う場合は 400 で拒否する（実効ロールが暗黙に max になるのを避ける）
    fn resolve_privileges(
        &self,
        rules: &[PrivilegeRule],
    ) -> impl Future<Output = Result<Vec<StoredPrivilege>, AppError>> {
        async move {
            // 名前解決の前に件数で弾く。解決はルールごとにカタログを引くので、
            // どうせ上限で拒否する入力に対して先に走らせない。
            if rules.len() > MAX_PRIVILEGE_RULES {
                return Err(AppError::InvalidPrivilege {
                    reason: format!(
                        "a user cannot hold more than {MAX_PRIVILEGE_RULES} privileges"
                    ),
                });
            }

            let mut resolved: Vec<StoredPrivilege> = Vec::with_capacity(rules.len());

            for rule in rules {
                let stored = match rule {
                    PrivilegeRule::Global { role } => StoredPrivilege::Global { role: *role },
                    PrivilegeRule::Database { db_name, role } => StoredPrivilege::Database {
                        db_id: self.require_database_id(db_name).await?,
                        role: *role,
                    },
                    PrivilegeRule::Table {
                        db_name,
                        table_name,
                        role,
                    } => {
                        let db_id = self.require_database_id(db_name).await?;
                        StoredPrivilege::Table {
                            db_id,
                            table_id: self.table_id(db_id, table_name).await?.ok_or_else(|| {
                                AppError::TableNotFound {
                                    name: table_name.clone(),
                                }
                            })?,
                            role: *role,
                        }
                    }
                };

                if let Some(existing) = resolved.iter().find(|e| e.target() == stored.target()) {
                    if existing.role() != stored.role() {
                        return Err(AppError::InvalidPrivilege {
                            reason: "conflicting roles were given for the same target".to_string(),
                        });
                    }
                    continue;
                }
                resolved.push(stored);
            }

            Ok(resolved)
        }
    }

    /// 1 件のルールを解決する。
    fn resolve_privilege(
        &self,
        rule: &PrivilegeRule,
    ) -> impl Future<Output = Result<StoredPrivilege, AppError>> {
        async move {
            Ok(self
                .resolve_privileges(std::slice::from_ref(rule))
                .await?
                .pop()
                .expect("resolving one rule yields one rule"))
        }
    }

    /// 適用対象を解決する。剥奪はロールを問わないので、対象キーだけを返す。
    fn resolve_target(
        &self,
        target: &PrivilegeTarget,
    ) -> impl Future<Output = Result<StoredTarget, AppError>> {
        async move {
            Ok(match target {
                PrivilegeTarget::Global => StoredTarget::Global,
                PrivilegeTarget::Database { db_name } => {
                    StoredTarget::Database(self.require_database_id(db_name).await?)
                }
                PrivilegeTarget::Table {
                    db_name,
                    table_name,
                } => {
                    let db_id = self.require_database_id(db_name).await?;
                    StoredTarget::Table(self.table_id(db_id, table_name).await?.ok_or_else(
                        || AppError::TableNotFound {
                            name: table_name.clone(),
                        },
                    )?)
                }
            })
        }
    }

    /// 参照先が既に消えているルールを取り除く。
    ///
    /// そうしたルールは認可判定で決して一致せず、取得時にも隠されるため無害だが、
    /// 残したままだと「付与 → 対象削除」を繰り返すぶんだけ配列が伸びていく。
    fn prune_dangling<'a>(
        &'a self,
        privileges: &'a mut Vec<StoredPrivilege>,
    ) -> impl Future<Output = Result<(), AppError>> + 'a {
        async move {
            let mut alive = Vec::with_capacity(privileges.len());
            for rule in privileges.iter() {
                let resolvable = match rule.target() {
                    StoredTarget::Global => true,
                    StoredTarget::Database(db_id) => self.database_name(db_id).await?.is_some(),
                    StoredTarget::Table(table_id) => self.table_name(table_id).await?.is_some(),
                };
                if resolvable {
                    alive.push(*rule);
                }
            }
            *privileges = alive;
            Ok(())
        }
    }

    /// 保存形式（ID ベース）を API 表現（名前ベース）へ描画する。
    ///
    /// 既に削除されたデータベース／テーブルを指すルールは名前へ解決できないため
    /// 取り除く。そうしたルールは認可判定でも決して一致しないので、隠すことで
    /// 「見えている権限 = 実際に効く権限」を保つ。
    fn render_privileges(
        &self,
        stored: &[StoredPrivilege],
    ) -> impl Future<Output = Result<Vec<PrivilegeRule>, AppError>> {
        async move {
            let mut out = Vec::with_capacity(stored.len());

            for rule in stored {
                let rendered = match rule {
                    StoredPrivilege::Global { role } => Some(PrivilegeRule::Global { role: *role }),
                    StoredPrivilege::Database { db_id, role } => self
                        .database_name(*db_id)
                        .await?
                        .map(|db_name| PrivilegeRule::Database {
                            db_name,
                            role: *role,
                        }),
                    StoredPrivilege::Table {
                        db_id,
                        table_id,
                        role,
                    } => match (
                        self.database_name(*db_id).await?,
                        self.table_name(*table_id).await?,
                    ) {
                        (Some(db_name), Some(table_name)) => Some(PrivilegeRule::Table {
                            db_name,
                            table_name,
                            role: *role,
                        }),
                        _ => None,
                    },
                };

                if let Some(rendered) = rendered {
                    out.push(rendered);
                }
            }

            Ok(out)
        }
    }
}

/// 読み取りトランザクション上で行える操作。
pub trait ReadRepository: MetaRepository {
    fn database_info(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<DatabaseInfoResponse>, AppError>>;

    /// Database の一覧を [`DatabaseId`] つきで取得する。
    /// 呼び出し側は権限の絞り込みに ID を使うため、引き直さずに済むよう ID を添えて返す。
    fn database_list(
        &self,
    ) -> impl Future<Output = Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError>>;

    fn table_info(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> impl Future<Output = Result<Option<Table>, AppError>>;

    fn table_list(&self, db_name: &str) -> impl Future<Output = Result<Vec<Table>, AppError>>;

    /// 既に ID を解決済みの呼び出し側が、名前からの引き直しを避けるために使う。
    fn table_list_by_id(
        &self,
        db_id: DatabaseId,
    ) -> impl Future<Output = Result<Vec<Table>, AppError>>;

    /// テーブルが保持する [`FlexId`] の総数を返す。
    fn table_count(&self, table_id: TableId) -> impl Future<Output = Result<u64, AppError>>;

    /// 指定された範囲の空間 ID を値ごとにグループ化して返す。
    fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> impl Future<Output = Result<ValueGroups, AppError>>;

    /// 値が `value` と等しいセルを引く（値インデックス経由）。
    fn data_filter_eq(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> impl Future<Output = Result<Vec<FlexId>, AppError>>;

    /// 値が `lo`〜`hi`（両端含む）に入るセルを引く。
    fn data_filter_range(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> impl Future<Output = Result<Vec<FlexId>, AppError>>;

    fn get_user(&self, username: &str) -> impl Future<Output = Result<Option<User>, AppError>>;

    fn require_user(&self, username: &str) -> impl Future<Output = Result<User, AppError>>;

    fn get_all_users(&self) -> impl Future<Output = Result<Vec<User>, AppError>>;
}

/// 書き込みトランザクション上で行える操作。
///
/// 読み取り系の一部（`database_info` / `table_info`）も併せ持つのは、作成前の重複確認など
/// 「同じトランザクション内で読んでから書く」処理が必要なため。
pub trait WriteRepository: MetaRepository {
    // --- 同一トランザクション内での確認用の読み取り ---

    fn database_info(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<DatabaseInfoResponse>, AppError>>;

    fn table_info(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> impl Future<Output = Result<Option<Table>, AppError>>;

    // --- データベース ---

    fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> impl Future<Output = Result<DatabaseInfoResponse, AppError>>;

    fn database_remove(&mut self, name: &str) -> impl Future<Output = Result<(), AppError>>;

    fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn database_copy(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> impl Future<Output = Result<DatabaseInfoResponse, AppError>>;

    // --- テーブル ---

    #[allow(clippy::too_many_arguments)]
    fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
    ) -> impl Future<Output = Result<Table, AppError>>;

    #[allow(clippy::too_many_arguments)]
    fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
        validate_existing_data: bool,
    ) -> impl Future<Output = Result<Table, AppError>>;

    fn table_remove(
        &mut self,
        db_name: &str,
        table_name: &str,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn table_copy(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> impl Future<Output = Result<Table, AppError>>;

    // --- データ ---

    fn data_insert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> impl Future<Output = Result<(), AppError>>;

    fn data_upsert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> impl Future<Output = Result<(), AppError>>;

    fn data_remove(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> impl Future<Output = Result<(), AppError>>;

    // --- ユーザーと権限 ---

    fn create_user(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> impl Future<Output = Result<(), AppError>>;

    fn set_password(
        &mut self,
        username: &str,
        password_hash: String,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn grant_privilege(
        &mut self,
        username: &str,
        rule: &PrivilegeRule,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn revoke_privilege(
        &mut self,
        username: &str,
        target: &PrivilegeTarget,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn delete_user(&mut self, username: &str) -> impl Future<Output = Result<(), AppError>>;
}

/// ストレージ本体。トランザクション境界を提供する。
///
/// `Clone` を要求するのは `AppState` に持たせて各リクエストへ配るため
/// （実装はハンドルの複製であり、データ本体は複製しない）。
pub trait Storage: Clone + Send + Sync + 'static {
    type Read<'a>: ReadRepository
    where
        Self: 'a;
    type Write<'a>: WriteRepository
    where
        Self: 'a;

    /// 読み取りトランザクションを開いてクロージャを実行する。
    ///
    /// 戻り値の Future に `Send` を課していないのは、バックエンドが feature で 1 つに
    /// 確定するため、具体型経由の呼び出しで auto-trait が漏れ出すのに任せる方が境界が
    /// 単純になるから（`AsyncFnOnce::CallOnceFuture` への HRTB 境界を書かずに済む）。
    fn read<T, F>(&self, f: F) -> impl Future<Output = Result<T, AppError>>
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
    fn write<T, F>(&self, f: F) -> impl Future<Output = Result<T, AppError>>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static;
}
