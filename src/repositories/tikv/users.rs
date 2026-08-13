//! ユーザーと権限の永続化。
//!
//! 対象は常に 1 ユーザーの 1 キーで範囲スキャンから変更を導く操作がないので、排他はユーザー名
//! 単位で足りる。権限が参照するデータベース・テーブルの解決はロックを取らない――参照先が
//! 消えたルールは隠して書き換えのついでに掃除する設計なので、スナップショットで十分。

use super::keys::{self, LockScope};
use crate::error::AppError;
use crate::models::users::{
    MAX_PRIVILEGE_RULES, PrivilegeRule, PrivilegeTarget, User, UserMetadata,
};
use crate::repositories::CatalogRepository;

use super::catalog::{decode, encode};
use super::kv::{Reader, Readers};
use super::{TikvRead, TikvWrite, kv};

pub(super) async fn user_meta<R: Reader>(
    txn: &Readers<R>,
    username: &str,
) -> Result<Option<UserMetadata>, AppError> {
    match kv::get(txn, keys::user(username)).await? {
        Some(bytes) => Ok(Some(decode("user", &bytes)?)),
        None => Ok(None),
    }
}

async fn put_user_meta(
    txn: &Readers<kv::LazyTxn>,
    username: &str,
    meta: &UserMetadata,
) -> Result<(), AppError> {
    kv::put(txn, keys::user(username), encode("user", meta)?).await;
    Ok(())
}

impl<R: Reader> TikvRead<'_, R> {
    #[tracing::instrument(skip_all)]
    pub(super) async fn get_all_users_impl(&self) -> Result<Vec<User>, AppError> {
        let entries = kv::scan_prefix(&self.txn, &keys::Ns::Users.prefix()).await?;
        entries
            .iter()
            .map(|(key, value)| {
                let username = keys::username_from_key(key)?;
                Ok(User::from_meta(username, decode("user", value)?))
            })
            .collect()
    }
}

impl TikvWrite<'_> {
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub(super) async fn create_user_impl(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<(), AppError> {
        // 「存在しないことを確認してから作る」ため排他する。
        self.require_lock(LockScope::User, username.as_bytes())?;

        if user_meta(&self.txn, username).await?.is_some() {
            return Err(AppError::Conflict("User already exists".to_string()));
        }
        let meta = UserMetadata {
            id,
            password_hash,
            token_version: 0,
            privileges: self.resolve_privileges(privileges).await?,
        };
        put_user_meta(&self.txn, username, &meta).await
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub(super) async fn set_password_impl(
        &mut self,
        username: &str,
        password_hash: String,
    ) -> Result<(), AppError> {
        self.require_lock(LockScope::User, username.as_bytes())?;

        let mut meta = self.require_user_meta(username).await?;
        meta.password_hash = password_hash;
        // 発行済みトークンを失効させる。
        meta.token_version = meta.token_version.wrapping_add(1);
        put_user_meta(&self.txn, username, &meta).await
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub(super) async fn grant_privilege_impl(
        &mut self,
        username: &str,
        rule: &PrivilegeRule,
    ) -> Result<(), AppError> {
        self.require_lock(LockScope::User, username.as_bytes())?;

        let stored = self.resolve_privilege(rule).await?;
        let mut meta = self.require_user_meta(username).await?;

        self.prune_dangling(&mut meta.privileges).await?;
        meta.privileges.retain(|r| r.target() != stored.target());

        if meta.privileges.len() >= MAX_PRIVILEGE_RULES {
            return Err(AppError::InvalidPrivilege {
                reason: format!("a user cannot hold more than {MAX_PRIVILEGE_RULES} privileges"),
            });
        }
        meta.privileges.push(stored);
        put_user_meta(&self.txn, username, &meta).await
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub(super) async fn revoke_privilege_impl(
        &mut self,
        username: &str,
        target: &PrivilegeTarget,
    ) -> Result<(), AppError> {
        self.require_lock(LockScope::User, username.as_bytes())?;

        let target = self.resolve_target(target).await?;
        let mut meta = self.require_user_meta(username).await?;

        self.prune_dangling(&mut meta.privileges).await?;
        let before = meta.privileges.len();
        meta.privileges.retain(|r| r.target() != target);
        if meta.privileges.len() == before {
            return Err(AppError::NotFound(
                "The user has no privilege for that target".into(),
            ));
        }

        put_user_meta(&self.txn, username, &meta).await
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub(super) async fn delete_user_impl(&mut self, username: &str) -> Result<(), AppError> {
        self.require_lock(LockScope::User, username.as_bytes())?;

        self.require_user_meta(username).await?;
        kv::delete(&self.txn, keys::user(username)).await;
        Ok(())
    }
}
