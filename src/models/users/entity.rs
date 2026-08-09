use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::id::{DatabaseId, TableId};

/// LMDB の `users` テーブルに保存されるユーザーの内部表現。
///
/// 過去バージョン（`is_global_admin` フィールドを持つ形式）とは非互換。
/// `deny_unknown_fields` を付けているため、旧形式のレコードは黙って
/// デフォルト値に落ちるのではなくパースエラーとして表面化する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserMetadata {
    pub id: Uuid,
    pub password_hash: String,
    /// トークンの世代番号。
    ///
    /// パスワード変更のように発行済みトークンを即座に失効させたい操作のたびに
    /// インクリメントする。JWT に埋め込んだ値と一致しないトークンは無効として扱う。
    ///
    /// 権限の変更ではインクリメントしない。認証ミドルウェアが毎リクエスト
    /// ユーザーを読み直しており、権限変更は次のリクエストから即座に反映されるため。
    pub token_version: u64,
    /// 保存されている権限ルール。データベース名・テーブル名ではなく ID を保持する。
    ///
    /// 1 つの対象につき高々 1 件。件数は [`MAX_PRIVILEGE_RULES`] で頭打ちにしている。
    pub privileges: Vec<StoredPrivilege>,
}

/// 1 ユーザーが保持できる権限ルールの上限。
///
/// 認証ミドルウェアが毎リクエストこの配列を含む JSON を読んでパースするため、
/// 際限なく増えるとリクエスト全体のレイテンシに響く。付与は対象ごとの追加なので、
/// 明示的な上限が無いと運用次第でいくらでも伸びうる。
pub const MAX_PRIVILEGE_RULES: usize = 1000;

/// 保存形式の権限ルール。
///
/// データベース名・テーブル名は改名・削除・再作成で意味が変わるため、
/// 権限は必ず UUID（[`DatabaseId`] / [`TableId`]）で保持する。これにより
///
/// - 改名しても権限はオブジェクトに追従する
/// - 削除して同名で作り直しても新しい UUID になるので、旧権限は決して一致しない
///
/// API 上の表現は名前ベースの [`PrivilegeRule`](crate::models::users::PrivilegeRule) で、
/// 付与時に名前 → ID、取得時に ID → 名前へ解決する。
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
            StoredPrivilege::Global { role } => *role,
            StoredPrivilege::Database { role, .. } | StoredPrivilege::Table { role, .. } => {
                UserRole::from(*role)
            }
        }
    }

    /// このルールが適用される対象。ロールを含まないので、付与の upsert と
    /// 剥奪の照合をどちらもこのキーで行える。
    pub fn target(&self) -> StoredTarget {
        match *self {
            StoredPrivilege::Global { .. } => StoredTarget::Global,
            StoredPrivilege::Database { db_id, .. } => StoredTarget::Database(db_id),
            StoredPrivilege::Table { table_id, .. } => StoredTarget::Table(table_id),
        }
    }
}

/// 解決済みの適用対象。1 ユーザーの権限ルールはこのキーで一意になる。
///
/// テーブルは `TableId` だけで一意に定まるため、`DatabaseId` は含めない。
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
            DataRole::Read => UserRole::Read,
            DataRole::Write => UserRole::Write,
            DataRole::Manage => UserRole::Manage,
        }
    }
}
