//! カタログへの点参照と、その上に載る権限ルールの変換。

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::error::{AppError, Resource};
use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{
    AclEntry, Grant, MAX_PRIVILEGES_PER_USER, PrivilegeRule, PrivilegeTarget, ResolvedPrivilege,
    ResolvedTarget, Scope, UserRecord, UserRole,
};

/// カタログと ACL への参照。
///
/// 実装が用意するのは**行を引く手段だけ**。認可の規則（どのロールがどのロールを含むか、
/// どのスコープがどこまで届くか）は [`User::allows`](crate::models::users::User::allows)
/// に 1 箇所だけあり、バックエンドごとに複製しない。
// `async fn` の Future に `Send` を課さない理由は [`Storage`](super::Storage) を参照。
#[allow(async_fn_in_trait)]
pub trait CatalogRepository {
    // --- 点参照 ---

    async fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError>;

    async fn table_id(
        &self,
        db_id: DatabaseId,
        table_name: &str,
    ) -> Result<Option<TableId>, AppError>;

    async fn user_record(&self, username: &str) -> Result<Option<UserRecord>, AppError>;

    // --- 一括参照 ---
    //
    // 名前の解決はバックエンドによっては 1 件ずつがネットワーク往復になる。権限の描画は
    // 保持数ぶん名前を要求するので、まとめて渡せる形が要る。

    async fn database_names(
        &self,
        ids: &[DatabaseId],
    ) -> Result<HashMap<DatabaseId, String>, AppError>;

    /// テーブル ID の逆引き。**所属データベースも一緒に返す。**
    ///
    /// 名前だけだと、権限の描画で「そのテーブルが本当にそのデータベースの配下か」を
    /// 確かめられない。
    async fn table_entries(
        &self,
        ids: &[TableId],
    ) -> Result<HashMap<TableId, (DatabaseId, String)>, AppError>;

    // --- ACL ---

    /// 対象スコープの判定に必要な行だけを引く。
    ///
    /// 何を読むかはスコープで決まる。[`Scope::Table`] は 2 行を 1 回の一括取得へ、
    /// [`Scope::AnyIn`] は 1 件だけの範囲検索へ落とせる。
    async fn grant_for(&self, principal: PrincipalId, scope: Scope) -> Result<Grant, AppError>;

    /// この主体が持つ全行。権限一覧の描画に使う。
    async fn acl_entries(&self, principal: PrincipalId) -> Result<Vec<AclEntry>, AppError>;

    /// この主体が何らかの権限を持つデータベース。
    ///
    /// データベース一覧を**権限の側から**引くために使う。全データベースを走査してから
    /// 捨てる形だと、権限を持たない対象のコストまで払うことになる。
    async fn acl_databases(&self, principal: PrincipalId)
    -> Result<BTreeSet<DatabaseId>, AppError>;

    /// この主体がこのデータベース配下に持つテーブル単位の行。
    ///
    /// データベーススコープの行は含まない（それを持つ利用者は全テーブルが見えるので、
    /// 呼び出し側が [`Grant::Database`] で先に決着させる）。
    async fn acl_tables_in(
        &self,
        principal: PrincipalId,
        db_id: DatabaseId,
    ) -> Result<BTreeSet<TableId>, AppError>;

    // --- 既定実装 ---

    async fn require_database_id(&self, name: &str) -> Result<DatabaseId, AppError> {
        self.database_id(name)
            .await?
            .ok_or_else(|| Resource::Database.not_found(name.to_string()))
    }

    async fn require_user_record(&self, username: &str) -> Result<UserRecord, AppError> {
        self.user_record(username)
            .await?
            .ok_or_else(|| Resource::User.not_found(username))
    }

    /// 剥奪はロールを問わないので、対象キーだけを返す。
    async fn resolve_target(&self, target: &PrivilegeTarget) -> Result<ResolvedTarget, AppError> {
        Ok(match target {
            PrivilegeTarget::Global => ResolvedTarget::Global,
            PrivilegeTarget::Database { db_name } => ResolvedTarget::Data(DataTarget::database(
                self.require_database_id(db_name).await?,
            )),
            PrivilegeTarget::Table {
                db_name,
                table_name,
            } => {
                let db_id = self.require_database_id(db_name).await?;
                let table_id = self
                    .table_id(db_id, table_name)
                    .await?
                    .ok_or_else(|| Resource::Table.not_found(table_name.clone()))?;
                ResolvedTarget::Data(DataTarget::table(db_id, table_id))
            }
        })
    }

    async fn resolve_rule(&self, rule: &PrivilegeRule) -> Result<ResolvedPrivilege, AppError> {
        Ok(match rule {
            PrivilegeRule::Global { role } => ResolvedPrivilege::Global(*role),
            PrivilegeRule::Database { db_name, role } => ResolvedPrivilege::Data {
                target: DataTarget::database(self.require_database_id(db_name).await?),
                role: *role,
            },
            PrivilegeRule::Table {
                db_name,
                table_name,
                role,
            } => {
                let db_id = self.require_database_id(db_name).await?;
                let table_id = self
                    .table_id(db_id, table_name)
                    .await?
                    .ok_or_else(|| Resource::Table.not_found(table_name.clone()))?;
                ResolvedPrivilege::Data {
                    target: DataTarget::table(db_id, table_id),
                    role: *role,
                }
            }
        })
    }

    /// API 表現（名前ベース）の権限ルール列を保存形式（ID ベース）へ解決する。
    ///
    /// 同じ対象を指すルールはロールが一致するときだけ畳む。食い違う場合に拒否するのは、
    /// 実効ロールが暗黙に max になるのを避けるため。
    async fn resolve_rules(
        &self,
        rules: &[PrivilegeRule],
    ) -> Result<Vec<ResolvedPrivilege>, AppError> {
        // 解決はルールごとにカタログを引くので、上限で拒否する入力には走らせない。
        if rules.len() > MAX_PRIVILEGES_PER_USER as usize {
            return Err(AppError::too_many_privileges());
        }

        let mut resolved: Vec<ResolvedPrivilege> = Vec::with_capacity(rules.len());
        let mut seen: HashSet<ResolvedTarget> = HashSet::with_capacity(rules.len());

        for rule in rules {
            let entry = self.resolve_rule(rule).await?;
            if !seen.insert(entry.target()) {
                // 同じ対象が 2 度出た。ロールが一致するときだけ黙って畳む。
                if resolved.contains(&entry) {
                    continue;
                }
                return Err(AppError::InvalidPrivilege {
                    reason: "conflicting roles were given for the same target".to_string(),
                });
            }
            resolved.push(entry);
        }
        Ok(resolved)
    }

    /// 保存形式（ID ベース）を API 表現（名前ベース）へ描画する。
    ///
    /// 名前の解決は**2 回の一括取得**で済ませる。保持数ぶん往復すると、権限を多く持つ
    /// 利用者ほど一覧が重くなる。
    ///
    /// 名前へ解決できない行を落とすのは、「見えている権限 = 実際に効く権限」を保つため。
    /// 対象を消すときに ACL 行も消しているので通常は起きないが、途中で失敗した掃除の
    /// 取り残しがあっても表示と実効権限は食い違わない。
    async fn render_privileges(
        &self,
        global_role: Option<UserRole>,
        entries: &[AclEntry],
    ) -> Result<Vec<PrivilegeRule>, AppError> {
        let mut out = Vec::with_capacity(entries.len() + usize::from(global_role.is_some()));
        if let Some(role) = global_role {
            out.push(PrivilegeRule::Global { role });
        }
        if entries.is_empty() {
            return Ok(out);
        }

        let db_ids: Vec<DatabaseId> = entries
            .iter()
            .map(|e| e.target.db_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let table_ids: Vec<TableId> = entries
            .iter()
            .filter_map(|e| e.target.table_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let db_names = self.database_names(&db_ids).await?;
        let tables = self.table_entries(&table_ids).await?;

        for AclEntry { target, role } in entries.iter().copied() {
            let Some(db_name) = db_names.get(&target.db_id) else {
                continue;
            };
            match target.table_id {
                None => out.push(PrivilegeRule::Database {
                    db_name: db_name.clone(),
                    role,
                }),
                // 所属が食い違う行は描画しない（認可でも一致しない）。
                Some(table_id) => {
                    if let Some((owner, table_name)) = tables.get(&table_id)
                        && *owner == target.db_id
                    {
                        out.push(PrivilegeRule::Table {
                            db_name: db_name.clone(),
                            table_name: table_name.clone(),
                            role,
                        });
                    }
                }
            }
        }
        Ok(out)
    }
}
