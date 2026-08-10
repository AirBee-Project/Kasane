//! カタログ（データベース・テーブル・ユーザーの定義）の参照と、
//! その上に載る権限ルールの変換。

use crate::error::AppError;
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::{
    MAX_PRIVILEGE_RULES, PrivilegeRule, PrivilegeTarget, StoredPrivilege, StoredTarget,
    UserMetadata,
};

/// 読み取り・書き込みどちらのトランザクションからも必要になる、カタログへの点参照。
///
/// 実装が用意するのは点参照だけで、その上に載る権限ルールの名前 ⇄ ID 変換は既定実装として
/// ここに 1 箇所だけ置く（認可の規則をバックエンドごとに複製しないため）。
// `async fn` の戻り値の Future には呼び出し側から `Send` 境界を付けられない。
// このアプリではバックエンドが feature で 1 つに確定し、Send 性は具体型経由で
// 漏れ出すため、trait 側で境界を要求する必要がない（`storage.rs` の設計メモを参照）。
// 署名の読みやすさを優先して `async fn` を使う。
#[allow(async_fn_in_trait)]
pub trait CatalogRepository {
    // --- 実装が用意する点参照 ---

    async fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError>;

    async fn table_id(
        &self,
        db_id: DatabaseId,
        table_name: &str,
    ) -> Result<Option<TableId>, AppError>;

    /// `DatabaseId` からデータベース名を引く。
    async fn database_name(&self, db_id: DatabaseId) -> Result<Option<String>, AppError>;

    /// `TableId` からテーブル名を引く。
    async fn table_name(&self, table_id: TableId) -> Result<Option<String>, AppError>;

    async fn user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError>;

    /// データベース配下のテーブル名を列挙する。
    async fn table_names(&self, db_id: DatabaseId) -> Result<Vec<String>, AppError>;

    // --- 上に載る共通ロジック ---

    async fn require_database_id(&self, name: &str) -> Result<DatabaseId, AppError> {
        self.database_id(name)
            .await?
            .ok_or_else(|| AppError::DatabaseNotFound {
                name: name.to_string(),
            })
    }

    async fn require_user_meta(&self, username: &str) -> Result<UserMetadata, AppError> {
        self.user_meta(username)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))
    }

    /// API 表現（名前ベース）の権限ルール列を保存形式（ID ベース）へ解決する。
    ///
    /// - 存在しないデータベース／テーブルを指すルールは 404 で拒否する
    /// - 同じ対象を指すルールが複数あればロールが一致する場合のみ 1 件に畳み、
    ///   食い違う場合は 400 で拒否する（実効ロールが暗黙に max になるのを避ける）
    async fn resolve_privileges(
        &self,
        rules: &[PrivilegeRule],
    ) -> Result<Vec<StoredPrivilege>, AppError> {
        // 名前解決の前に件数で弾く。解決はルールごとにカタログを引くので、
        // どうせ上限で拒否する入力に対して先に走らせない。
        if rules.len() > MAX_PRIVILEGE_RULES {
            return Err(AppError::InvalidPrivilege {
                reason: format!("a user cannot hold more than {MAX_PRIVILEGE_RULES} privileges"),
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

    /// 1 件のルールを解決する。
    async fn resolve_privilege(&self, rule: &PrivilegeRule) -> Result<StoredPrivilege, AppError> {
        Ok(self
            .resolve_privileges(std::slice::from_ref(rule))
            .await?
            .pop()
            .expect("resolving one rule yields one rule"))
    }

    /// 適用対象を解決する。剥奪はロールを問わないので、対象キーだけを返す。
    async fn resolve_target(&self, target: &PrivilegeTarget) -> Result<StoredTarget, AppError> {
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
                StoredTarget::Table(self.table_id(db_id, table_name).await?.ok_or_else(|| {
                    AppError::TableNotFound {
                        name: table_name.clone(),
                    }
                })?)
            }
        })
    }

    /// 参照先が既に消えているルールを取り除く。
    ///
    /// そうしたルールは認可判定で決して一致せず、取得時にも隠されるため無害だが、
    /// 残したままだと「付与 → 対象削除」を繰り返すぶんだけ配列が伸びていく。
    async fn prune_dangling(&self, privileges: &mut Vec<StoredPrivilege>) -> Result<(), AppError> {
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

    /// 保存形式（ID ベース）を API 表現（名前ベース）へ描画する。
    ///
    /// 既に削除されたデータベース／テーブルを指すルールは名前へ解決できないため
    /// 取り除く。そうしたルールは認可判定でも決して一致しないので、隠すことで
    /// 「見えている権限 = 実際に効く権限」を保つ。
    async fn render_privileges(
        &self,
        stored: &[StoredPrivilege],
    ) -> Result<Vec<PrivilegeRule>, AppError> {
        let mut out = Vec::with_capacity(stored.len());

        for rule in stored {
            let rendered = match rule {
                StoredPrivilege::Global { role } => Some(PrivilegeRule::Global { role: *role }),
                StoredPrivilege::Database { db_id, role } => {
                    self.database_name(*db_id)
                        .await?
                        .map(|db_name| PrivilegeRule::Database {
                            db_name,
                            role: *role,
                        })
                }
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
