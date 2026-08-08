use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::entity::{StoredPrivilege, UserMetadata, UserRole};
use super::privilege::Scope;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub id: Uuid,
    pub token_version: u64,
    pub privileges: Vec<StoredPrivilege>,
}

impl User {
    pub fn from_meta(username: &str, meta: UserMetadata) -> Self {
        Self {
            username: username.to_string(),
            id: meta.id,
            token_version: meta.token_version,
            privileges: meta.privileges,
        }
    }

    /// `Global` スコープのルールだけで到達できる最大ロール。
    pub fn global_role(&self) -> Option<UserRole> {
        self.privileges
            .iter()
            .filter_map(|rule| match rule {
                StoredPrivilege::Global { role } => Some(*role),
                _ => None,
            })
            .max()
    }

    /// `Global` スコープで `required` 以上のロールを持つか。
    pub fn has_global_role(&self, required: UserRole) -> bool {
        self.global_role().is_some_and(|role| role >= required)
    }

    /// サーバーの管理者（ユーザーの作成・削除・権限付与ができる）か。
    ///
    /// データ面の「全データベースに対する Manage」（`Global { role: Manage }`）とは別物で、
    /// 制御面の権限は `Global { role: Admin }` だけが持つ。
    pub fn is_global_admin(&self) -> bool {
        self.has_global_role(UserRole::Admin)
    }

    /// 対象スコープに対する実効ロール。該当するルールが 1 つも無ければ `None`。
    fn effective_role(&self, scope: Scope) -> Option<UserRole> {
        self.privileges
            .iter()
            .filter_map(|rule| match (rule, scope) {
                (StoredPrivilege::Global { role }, _) => Some(*role),

                // データベース単位のルールは、そのデータベースを指すどのスコープにも効く。
                (
                    StoredPrivilege::Database { db_id, role },
                    Scope::Database(target) | Scope::Table(target, _) | Scope::AnyIn(target),
                ) if *db_id == target => Some(*role),

                // テーブル単位のルールは、対象がそのテーブル自身であるか、
                // 「配下のどれかに触れれば足りる」判定のときだけ効く。
                (StoredPrivilege::Table { table_id, role, .. }, Scope::Table(_, target))
                    if *table_id == target =>
                {
                    Some(*role)
                }
                (StoredPrivilege::Table { db_id, role, .. }, Scope::AnyIn(target))
                    if *db_id == target =>
                {
                    Some(*role)
                }

                _ => None,
            })
            .max()
    }

    /// 対象スコープに対して `required` 以上の権限を持つか。
    ///
    /// [`Scope::AnyIn`] は「配下のどれかに届く」という閲覧向けの緩い判定なので、
    /// [`UserRole::Read`] より上の要求を満たすことはない。テーブル 1 つへの Manage が
    /// データベース全体への Manage に化けるのを防ぐため。
    pub fn can(&self, scope: Scope, required: UserRole) -> bool {
        if matches!(scope, Scope::AnyIn(_)) && required > UserRole::Read {
            return false;
        }
        self.effective_role(scope)
            .is_some_and(|role| role >= required)
    }
}
