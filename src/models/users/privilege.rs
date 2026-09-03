use serde::{Deserialize, Serialize};

use super::entity::{DataRole, UserRole};
use crate::models::id::{DataTarget, DatabaseId, TableId};

/// 認可判定の対象。どの ACL 行を読むかはこれで決まり、結果は
/// [`Grant`](super::Grant) として返る。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// データベース全体に対する操作。テーブル単位の行では満たせない。
    Database(DatabaseId),
    /// 特定のテーブル 1 つに対する操作。
    Table(DatabaseId, TableId),
    /// 配下のどれかに届けば足りる操作（存在確認・一覧）。
    AnyIn(DatabaseId),
}

/// API 上の権限ルール表現。
///
/// 保存形式（ACL 行）は UUID ベースだが、外部にはデータベース名・テーブル名で見せる。
/// 付与時に名前 → ID を解決し、存在しない名前は 404 で弾く。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum PrivilegeRule {
    /// サーバー全体。`role` に `admin` を指定できるのはこのスコープだけ。
    Global { role: UserRole },
    /// データベース配下すべて。`admin` は指定できない。
    Database { db_name: String, role: DataRole },
    /// 単一テーブル。`admin` は指定できない。
    Table {
        db_name: String,
        table_name: String,
        role: DataRole,
    },
}

impl PrivilegeRule {
    /// ロールを除いた適用対象。同じ対象への重複指定を検出するのに使う。
    pub fn target(&self) -> PrivilegeTarget {
        match self {
            Self::Global { .. } => PrivilegeTarget::Global,
            Self::Database { db_name, .. } => PrivilegeTarget::Database {
                db_name: db_name.clone(),
            },
            Self::Table {
                db_name,
                table_name,
                ..
            } => PrivilegeTarget::Table {
                db_name: db_name.clone(),
                table_name: table_name.clone(),
            },
        }
    }
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

/// 名前を ID へ解決した [`PrivilegeTarget`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResolvedTarget {
    Global,
    /// ACL 行として保存される対象。
    Data(DataTarget),
}

/// 名前を ID へ解決した [`PrivilegeRule`]。
///
/// **スコープごとにロールの型が違う。** `global` だけが `admin` を持ちえて、データ面は
/// [`DataRole`] しか持ちえない。この区別を解決後も型で保つので、保存する直前に
/// 「`admin` ではないこと」を実行時へ確かめ直す必要が無い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedPrivilege {
    Global(UserRole),
    Data { target: DataTarget, role: DataRole },
}

impl ResolvedPrivilege {
    pub fn target(&self) -> ResolvedTarget {
        match *self {
            Self::Global(_) => ResolvedTarget::Global,
            Self::Data { target, .. } => ResolvedTarget::Data(target),
        }
    }
}
