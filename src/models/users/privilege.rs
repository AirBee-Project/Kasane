use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::entity::UserRole;
use crate::models::id::{DatabaseId, TableId};

/// 認可判定の対象。どのルールが効くかはスコープごとに違う。
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// データベース全体に対する操作（改名・DB 単位の設定変更・新規テーブル作成など）。
    /// テーブル単位のルールでは満たせない。
    Database(DatabaseId),
    /// 特定のテーブル 1 つに対する操作。
    Table(DatabaseId, TableId),
    /// データベース配下のどれかにアクセスできれば足りる操作（存在確認・一覧）。
    /// テーブル単位のルールしか持たないユーザーでも通す。
    ///
    /// 「配下のどれか」で足りるのは閲覧のためだけなので、このスコープは
    /// [`UserRole::Read`] より上を満たすことはない（[`User::can`](crate::models::users::User::can) 参照）。
    AnyIn(DatabaseId),
}

/// API 上の権限ルール表現。
///
/// 保存形式（[`StoredPrivilege`](super::entity::StoredPrivilege)）は UUID ベースだが、
/// 外部にはデータベース名・テーブル名で見せる。付与時に名前 → ID を解決し、
/// 存在しない名前は 404 で弾く。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum PrivilegeRule {
    /// サーバー全体。`role` に `admin` を指定できるのはこのスコープだけ。
    #[schema(title = "GlobalPrivilege")]
    Global { role: UserRole },
    /// データベース配下すべて。
    #[schema(title = "DatabasePrivilege")]
    Database {
        #[schema(example = "example_database")]
        db_name: String,
        role: UserRole,
    },
    /// 単一テーブル。
    #[schema(title = "TablePrivilege")]
    Table {
        #[schema(example = "example_database")]
        db_name: String,
        #[schema(example = "example_table")]
        table_name: String,
        role: UserRole,
    },
}

/// 権限の適用対象（ロールを含まない）。
///
/// サブリソースのパスから組み立てる。剥奪はロールを問わず対象ごと落とすので、
/// 「Manage を指定したが実際は Read だったので何も消えなかった」が起こらない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivilegeTarget {
    Global,
    Database { db_name: String },
    Table { db_name: String, table_name: String },
}

impl PrivilegeTarget {
    /// 対象にロールを与えて 1 件のルールにする。
    pub fn with_role(self, role: UserRole) -> PrivilegeRule {
        match self {
            PrivilegeTarget::Global => PrivilegeRule::Global { role },
            PrivilegeTarget::Database { db_name } => PrivilegeRule::Database { db_name, role },
            PrivilegeTarget::Table {
                db_name,
                table_name,
            } => PrivilegeRule::Table {
                db_name,
                table_name,
                role,
            },
        }
    }
}

impl PrivilegeRule {
    pub fn role(&self) -> UserRole {
        match self {
            PrivilegeRule::Global { role }
            | PrivilegeRule::Database { role, .. }
            | PrivilegeRule::Table { role, .. } => *role,
        }
    }
}
