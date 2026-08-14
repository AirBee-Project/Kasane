use serde::{Deserialize, Serialize};

use super::entity::{DataRole, UserRecord, UserRole};
use crate::models::id::PrincipalId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub id: PrincipalId,
    pub token_version: u64,
    pub global_role: Option<UserRole>,
}

impl User {
    pub fn from_record(username: &str, record: UserRecord) -> Self {
        Self {
            username: username.to_string(),
            id: record.id,
            token_version: record.token_version,
            global_role: record.global_role,
        }
    }

    /// `global` スコープで `required` 以上のロールを持つか。
    ///
    /// **ACL を引かずに答えられる。** 呼び出し側はこれを先に試して、通るならカタログにも
    /// ACL にも触れずに済ませる。
    pub fn has_global_role(&self, required: UserRole) -> bool {
        self.global_role.is_some_and(|role| role >= required)
    }

    /// 制御面（ユーザーの作成・削除・権限付与）の権限。
    ///
    /// データ面の `global` / `manage` とは別物で、`global` / `admin` だけが持つ。
    pub fn is_global_admin(&self) -> bool {
        self.has_global_role(UserRole::Admin)
    }

    /// 引いてきた ACL 行から、対象スコープに対して `required` 以上を満たすか判定する。
    ///
    /// 認可の規則はここ 1 箇所だけにある。バックエンドが持つのは行を引く手段だけで、
    /// 強さの比較には関わらない。
    pub fn allows(&self, grant: &Grant, required: UserRole) -> bool {
        if self.has_global_role(required) {
            return true;
        }
        grant
            .effective_role()
            .is_some_and(|role| role >= required && grant.may_reach(required))
    }
}

/// 判定対象のスコープについてカタログから引いた ACL 行。
///
/// スコープごとに読む行が違うので、種類を型で分けてある。どの行を読むかは
/// [`Scope`](super::Scope) と 1 対 1 に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    /// データベース全体に対する操作。`acl[principal][db][DATABASE_SCOPE]` だけが効く。
    ///
    /// テーブル単位の行では満たせない。テーブル 1 つへの Manage が
    /// データベース全体への Manage に化けるのを防ぐため。
    Database(Option<DataRole>),
    /// 特定のテーブル 1 つ。データベーススコープの行も配下として効く。
    Table {
        database: Option<DataRole>,
        table: Option<DataRole>,
    },
    /// 配下のどれかに届けば足りる操作（存在確認・一覧）。
    ///
    /// `acl[principal][db]` に 1 行でもあれば真。テーブル単位の行しか持たない利用者でも
    /// 通る代わりに、[`UserRole::Read`] より上は決して満たさない。
    AnyIn(bool),
}

impl Grant {
    fn effective_role(&self) -> Option<UserRole> {
        match self {
            Self::Database(role) => role.map(UserRole::from),
            Self::Table { database, table } => [database, table]
                .into_iter()
                .flatten()
                .copied()
                .max()
                .map(UserRole::from),
            // 中身のロールは見ない（[`may_reach`] で Read 止まりにする）。
            Self::AnyIn(present) => present.then_some(UserRole::Read),
        }
    }

    /// このスコープが `required` を満たしうるか。
    ///
    /// [`Grant::AnyIn`] だけが Read で頭打ちになる。
    fn may_reach(&self, required: UserRole) -> bool {
        !matches!(self, Self::AnyIn(_)) || required <= UserRole::Read
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(global: Option<UserRole>) -> User {
        User {
            username: "u".to_string(),
            id: PrincipalId(uuid::Uuid::nil()),
            token_version: 0,
            global_role: global,
        }
    }

    #[test]
    fn global_role_short_circuits_every_scope() {
        let admin = user(Some(UserRole::Admin));
        // 行が 1 つも無くても通る。
        assert!(admin.allows(&Grant::Database(None), UserRole::Manage));
        assert!(admin.allows(
            &Grant::Table {
                database: None,
                table: None
            },
            UserRole::Manage
        ));
    }

    #[test]
    fn database_scope_ignores_table_rows() {
        let u = user(None);
        // テーブル単位の Manage しか無い利用者は、データベース全体の操作を通せない。
        assert!(!u.allows(&Grant::Database(None), UserRole::Manage));
    }

    #[test]
    fn table_scope_takes_the_stronger_of_the_two_rows() {
        let u = user(None);
        let grant = Grant::Table {
            database: Some(DataRole::Read),
            table: Some(DataRole::Manage),
        };
        assert!(u.allows(&grant, UserRole::Manage));

        let grant = Grant::Table {
            database: Some(DataRole::Manage),
            table: Some(DataRole::Read),
        };
        assert!(u.allows(&grant, UserRole::Manage));
    }

    #[test]
    fn any_in_never_satisfies_more_than_read() {
        let u = user(None);
        assert!(u.allows(&Grant::AnyIn(true), UserRole::Read));
        assert!(!u.allows(&Grant::AnyIn(true), UserRole::Write));
        assert!(!u.allows(&Grant::AnyIn(true), UserRole::Manage));
        assert!(!u.allows(&Grant::AnyIn(false), UserRole::Read));
    }

    #[test]
    fn global_read_does_not_grant_write() {
        let u = user(Some(UserRole::Read));
        assert!(u.allows(&Grant::Database(None), UserRole::Read));
        assert!(!u.allows(&Grant::Database(None), UserRole::Write));
    }

    /// 全体権限は `AnyIn` の頭打ちに引っかからない（頭打ちは ACL 行の話）。
    #[test]
    fn global_role_is_not_capped_by_any_in() {
        let u = user(Some(UserRole::Manage));
        assert!(u.allows(&Grant::AnyIn(false), UserRole::Manage));
    }
}
