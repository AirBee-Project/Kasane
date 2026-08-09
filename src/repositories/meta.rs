//! メタデータの読み取りと、権限ルールの名前 ⇄ UUID 変換。
//!
//! 権限は UUID で保存し API では名前で見せるため、この変換はユーザー権限を
//! 読み書きするすべての経路（付与・取得・リクエスト時の認可判定）から必要になる。
//! 読み取りトランザクションと書き込みトランザクションのどちらからも同じロジックを
//! 使えるよう、点参照だけを [`MetaRead`] の必須メソッドとして切り出し、その上の
//! 変換ロジックは既定実装として 1 箇所に置いている。
//!
//! ストレージ実装を差し替える（TiKV 等）場合の境界もここになる。全走査に依存する
//! 逆引きは持たず、`database_id_index` / `table_id_index` への点参照だけで済ませている。

use heed::BytesDecode;

use crate::db_init::DbIdAndName;
use crate::error::AppError;
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::{
    MAX_PRIVILEGE_RULES, PrivilegeRule, PrivilegeTarget, StoredPrivilege, StoredTarget,
    UserMetadata,
};
use crate::repositories::{KasaneDbRead, KasaneDbWrite};

pub trait MetaRead {
    // --- トランザクション種別ごとに実装する点参照 ---

    fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError>;
    fn table_id(&self, db_id: DatabaseId, table_name: &str) -> Result<Option<TableId>, AppError>;
    /// `DatabaseId` からデータベース名を引く（`database_id_index` への点参照）。
    fn database_name(&self, db_id: DatabaseId) -> Result<Option<String>, AppError>;
    /// `TableId` からテーブル名を引く（`table_id_index` への点参照）。
    fn table_name(&self, table_id: TableId) -> Result<Option<String>, AppError>;
    fn user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError>;
    /// データベース配下のテーブル名を列挙する。
    fn table_names(&self, db_id: DatabaseId) -> Result<Vec<String>, AppError>;

    // --- 上に載る共通ロジック ---

    fn require_database_id(&self, name: &str) -> Result<DatabaseId, AppError> {
        self.database_id(name)?
            .ok_or_else(|| AppError::DatabaseNotFound {
                name: name.to_string(),
            })
    }

    fn require_user_meta(&self, username: &str) -> Result<UserMetadata, AppError> {
        self.user_meta(username)?
            .ok_or_else(|| AppError::NotFound("User not found".into()))
    }

    /// API 表現（名前ベース）の権限ルール列を保存形式（ID ベース）へ解決する。
    ///
    /// - 存在しないデータベース／テーブルを指すルールは 404 で拒否する
    /// - 同じ対象を指すルールが複数あればロールが一致する場合のみ 1 件に畳み、
    ///   食い違う場合は 400 で拒否する（実効ロールが暗黙に max になるのを避ける）
    fn resolve_privileges(
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
                    db_id: self.require_database_id(db_name)?,
                    role: *role,
                },
                PrivilegeRule::Table {
                    db_name,
                    table_name,
                    role,
                } => {
                    let db_id = self.require_database_id(db_name)?;
                    StoredPrivilege::Table {
                        db_id,
                        table_id: self.table_id(db_id, table_name)?.ok_or_else(|| {
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
    fn resolve_privilege(&self, rule: &PrivilegeRule) -> Result<StoredPrivilege, AppError> {
        Ok(self
            .resolve_privileges(std::slice::from_ref(rule))?
            .pop()
            .expect("resolving one rule yields one rule"))
    }

    /// 適用対象を解決する。剥奪はロールを問わないので、対象キーだけを返す。
    fn resolve_target(&self, target: &PrivilegeTarget) -> Result<StoredTarget, AppError> {
        Ok(match target {
            PrivilegeTarget::Global => StoredTarget::Global,
            PrivilegeTarget::Database { db_name } => {
                StoredTarget::Database(self.require_database_id(db_name)?)
            }
            PrivilegeTarget::Table {
                db_name,
                table_name,
            } => {
                let db_id = self.require_database_id(db_name)?;
                StoredTarget::Table(self.table_id(db_id, table_name)?.ok_or_else(|| {
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
    /// 権限を書き換えるついでに落として、実際に効くルールだけを保存に残す。
    fn prune_dangling(&self, privileges: &mut Vec<StoredPrivilege>) -> Result<(), AppError> {
        let mut alive = Vec::with_capacity(privileges.len());
        for rule in privileges.iter() {
            let resolvable = match rule.target() {
                StoredTarget::Global => true,
                StoredTarget::Database(db_id) => self.database_name(db_id)?.is_some(),
                StoredTarget::Table(table_id) => self.table_name(table_id)?.is_some(),
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
    fn render_privileges(
        &self,
        stored: &[StoredPrivilege],
    ) -> Result<Vec<PrivilegeRule>, AppError> {
        let mut out = Vec::with_capacity(stored.len());

        for rule in stored {
            let rendered = match rule {
                StoredPrivilege::Global { role } => Some(PrivilegeRule::Global { role: *role }),
                StoredPrivilege::Database { db_id, role } => {
                    self.database_name(*db_id)?
                        .map(|db_name| PrivilegeRule::Database {
                            db_name,
                            role: *role,
                        })
                }
                StoredPrivilege::Table {
                    db_id,
                    table_id,
                    role,
                } => match (self.database_name(*db_id)?, self.table_name(*table_id)?) {
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

/// 点参照 5 つを、トランザクションを保持するフィールド名を指定して実装する。
macro_rules! impl_meta_read {
    ($target:ty, $txn:ident) => {
        impl MetaRead for $target {
            fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError> {
                if name.is_empty() {
                    return Ok(None);
                }
                Ok(self.db.databases.get(&self.$txn, name)?.map(|meta| meta.id))
            }

            fn table_id(
                &self,
                db_id: DatabaseId,
                table_name: &str,
            ) -> Result<Option<TableId>, AppError> {
                if table_name.is_empty() {
                    return Ok(None);
                }
                Ok(self
                    .db
                    .tables
                    .get(&self.$txn, &(db_id, table_name))?
                    .map(|meta| meta.id))
            }

            fn database_name(&self, db_id: DatabaseId) -> Result<Option<String>, AppError> {
                Ok(self
                    .db
                    .database_id_index
                    .get(&self.$txn, &db_id)?
                    .map(str::to_string))
            }

            fn table_name(&self, table_id: TableId) -> Result<Option<String>, AppError> {
                Ok(self
                    .db
                    .table_id_index
                    .get(&self.$txn, &table_id)?
                    .map(str::to_string))
            }

            fn user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError> {
                match self.db.users.get(&self.$txn, username)? {
                    Some(val) => Ok(Some(serde_json::from_str(val).map_err(|_| {
                        AppError::InternalError("Failed to parse user metadata".into())
                    })?)),
                    None => Ok(None),
                }
            }

            fn table_names(&self, db_id: DatabaseId) -> Result<Vec<String>, AppError> {
                let prefix = db_id.into_bytes();
                let mut names = Vec::new();
                for item in self
                    .db
                    .tables
                    .remap_types::<heed::types::Bytes, heed::types::Bytes>()
                    .prefix_iter(&self.$txn, prefix.as_slice())?
                {
                    let (k_bytes, _) = item?;
                    let (_, name) = DbIdAndName::bytes_decode(k_bytes).map_err(|e| {
                        AppError::InternalError(format!("Failed to decode table key: {e}"))
                    })?;
                    names.push(name.to_string());
                }
                Ok(names)
            }
        }
    };
}

impl_meta_read!(KasaneDbRead<'_>, read_txn);
impl_meta_read!(KasaneDbWrite<'_>, write_txn);
