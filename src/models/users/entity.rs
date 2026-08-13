use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::id::{DatabaseId, TableId};

/// 保存されるユーザーの内部表現。
///
/// `deny_unknown_fields` を付けているので、非互換なレコードは黙ってデフォルト値へ落ちず
/// パースエラーとして表面化する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserMetadata {
    pub id: Uuid,
    pub password_hash: String,
    /// トークンの世代番号。JWT に埋めた値と一致しないトークンは無効。
    ///
    /// 発行済みトークンを即座に失効させたい操作（パスワード変更など）で進める。権限変更で
    /// 進めないのは、認証ミドルウェアが毎リクエストユーザーを読み直しているため。
    pub token_version: u64,
    /// 1 つの対象につき高々 1 件。件数は [`MAX_PRIVILEGE_RULES`] で頭打ち。
    pub privileges: Vec<StoredPrivilege>,
}

/// 1 ユーザーが保持できる権限ルールの上限。
///
/// 認証ミドルウェアが毎リクエストこの配列を含む JSON をパースするので、際限なく増えると
/// リクエスト全体のレイテンシに響く。
pub const MAX_PRIVILEGE_RULES: usize = 1000;

/// 保存形式の権限ルール。名前ではなく UUID で保持する。
///
/// 名前は改名・削除・再作成で意味が変わる。UUID なら改名しても権限が追従し、同名で作り直しても
/// 新しい UUID になるので旧権限は決して一致しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum StoredPrivilege {
    /// サーバー全体に対する権限。
    Global { role: UserRole },
    /// 特定のデータベース配下すべてに対する権限。
    Database { db_id: DatabaseId, role: DataRole },
    /// 特定のテーブル 1 つに対する権限。
    Table {
        db_id: DatabaseId,
        table_id: TableId,
        role: DataRole,
    },
}

impl StoredPrivilege {
    pub fn role(&self) -> UserRole {
        match self {
            Self::Global { role } => *role,
            Self::Database { role, .. } | Self::Table { role, .. } => UserRole::from(*role),
        }
    }

    /// ロールを含まないので、付与の upsert と剥奪の照合を同じキーで行える。
    pub fn target(&self) -> StoredTarget {
        match *self {
            Self::Global { .. } => StoredTarget::Global,
            Self::Database { db_id, .. } => StoredTarget::Database(db_id),
            Self::Table { table_id, .. } => StoredTarget::Table(table_id),
        }
    }
}

/// 1 ユーザーの権限ルールはこのキーで一意になる。
///
/// テーブルは `TableId` だけで一意に定まるので `DatabaseId` は含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredTarget {
    Global,
    Database(DatabaseId),
    Table(TableId),
}

/// データ面のロール。データベース・テーブルスコープで指定できるのはこの 3 つだけ。
///
/// 制御面の `admin` を含まないため、`database` / `table` スコープに `admin` を
/// 与えるリクエストは型として表現できない（デシリアライズの時点で弾かれる）。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum DataRole {
    Read = 1,
    Write = 2,
    Manage = 3,
}

/// 権限の強さ。数値が大きいほど強く、上位は下位をすべて含む。
///
/// `admin` は制御面（ユーザーの作成・削除・権限付与）を表し、`global` スコープに
/// のみ現れる。データベース・テーブルスコープの保存・API 表現は [`DataRole`] を使う。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum UserRole {
    Read = 1,
    Write = 2,
    Manage = 3,
    Admin = 4,
}

impl From<DataRole> for UserRole {
    fn from(role: DataRole) -> Self {
        match role {
            DataRole::Read => Self::Read,
            DataRole::Write => Self::Write,
            DataRole::Manage => Self::Manage,
        }
    }
}
