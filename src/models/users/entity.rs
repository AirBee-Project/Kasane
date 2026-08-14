use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{AppError, Stored};
use crate::models::id::{DataTarget, PrincipalId};

/// 保存される利用者の内部表現。
///
/// **権限ルールはここに入らない。** 対象ごとに独立した ACL 行として持つ（[`AclEntry`]）。
/// 認証ミドルウェアが毎リクエスト読むのはこのレコードなので、保持する権限の数に
/// よらず一定の大きさに保つ必要がある。
///
/// `deny_unknown_fields` を付けているので、非互換なレコードは黙ってデフォルト値へ落ちず
/// パースエラーとして表面化する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserRecord {
    pub id: PrincipalId,
    pub password_hash: String,
    /// トークンの世代番号。JWT に埋めた値と一致しないトークンは無効。
    ///
    /// 発行済みトークンを即座に失効させたい操作（パスワード変更など）で進める。権限変更で
    /// 進めないのは、認証ミドルウェアが毎リクエストこのレコードを読み直しているため。
    pub token_version: u64,
    /// サーバー全体に対するロール。利用者ごとに高々 1 つなので行にせず埋め込む。
    ///
    /// ここに置くと [`has_global_role`](super::User::has_global_role) が I/O ゼロになり、
    /// 全体権限で通る要求はカタログにも ACL にも触れずに決着する。
    pub global_role: Option<UserRole>,
}

/// 1 利用者が保持できる ACL 行の上限。
///
/// 行が独立したので配列を舐める必要は無くなったが、上限そのものは残す。無制限だと
/// 権限一覧の描画と、対象を消すときの逆引き削除が青天井になる。
pub const MAX_PRIVILEGES_PER_USER: u32 = 50_000;

/// ACL の 1 行。保存形式では名前ではなく UUID で持つ。
///
/// 名前は改名・削除・再作成で意味が変わる。UUID なら改名しても権限が追従し、同名で
/// 作り直しても新しい UUID になるので旧権限は決して一致しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AclEntry {
    pub target: DataTarget,
    pub role: DataRole,
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

/// ACL 行の値は 1 バイト。serde を通さないのは、1 バイトのために形式の取り決めを
/// 増やす意味が無いため。
impl From<DataRole> for u8 {
    fn from(role: DataRole) -> Self {
        role as u8
    }
}

impl TryFrom<u8> for DataRole {
    type Error = AppError;

    fn try_from(byte: u8) -> Result<Self, AppError> {
        match byte {
            1 => Ok(Self::Read),
            2 => Ok(Self::Write),
            3 => Ok(Self::Manage),
            _ => Err(AppError::corrupt(
                Stored::AclRow,
                format!("unknown role byte {byte}"),
            )),
        }
    }
}

/// `admin` はデータ面に存在しないので、絞り込みは失敗しうる。
impl TryFrom<UserRole> for DataRole {
    type Error = AppError;

    fn try_from(role: UserRole) -> Result<Self, AppError> {
        match role {
            UserRole::Read => Ok(Self::Read),
            UserRole::Write => Ok(Self::Write),
            UserRole::Manage => Ok(Self::Manage),
            UserRole::Admin => Err(AppError::InvalidPrivilege {
                reason: "the admin role can only be granted on the global scope".to_string(),
            }),
        }
    }
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

/// 一覧で 1 利用者について返す概要。
///
/// 権限そのものを含めないのは、一覧のページあたりの読み取りを利用者数に比例させない
/// ため。詳細は `GET /users/{username}/privileges` で引く。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserSummary {
    #[schema(example = "example_user")]
    pub username: String,
    /// サーバー全体のロール。持たなければ `null`。
    pub global_role: Option<UserRole>,
}
