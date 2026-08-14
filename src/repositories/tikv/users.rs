//! 利用者レコードと ACL 行の永続化。
//!
//! 対象は常に 1 利用者の 1 キー、あるいは 1 対象の 1 行で、範囲スキャンから変更を導く
//! 操作が無いので、排他は利用者名の単位で足りる。ACL 行が参照するデータベース・
//! テーブルの解決はロックを取らない――対象を消すときに行ごと落としているので、
//! 読む側はスナップショットで十分。
//!
//! 付与・剥奪の意味論は [`WriteRepository`](crate::repositories::WriteRepository) の
//! 既定実装が持つ。ここにあるのは行を読み書きする手段だけ。

use std::collections::BTreeSet;

use super::keys::{self, LockScope};
use crate::error::{AppError, Stored};
use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{AclEntry, Grant, Scope, UserRecord};
use crate::repositories::encoding::acl::{AclKey, decode_role};

use super::catalog::{decode, encode};
use super::kv::{Reader, Readers};
use super::{TikvRead, TikvWrite, kv};

pub(super) async fn user_record<R: Reader>(
    txn: &Readers<R>,
    username: &str,
) -> Result<Option<UserRecord>, AppError> {
    match kv::get(txn, keys::user(username)).await? {
        Some(bytes) => Ok(Some(decode(Stored::UserRecord, &bytes)?)),
        None => Ok(None),
    }
}

/// スコープの判定に必要な行だけを読む。
///
/// [`Scope::Table`] は 2 行を **1 回の `batch_get`** に束ねる。順に引くと往復が 2 回に
/// なるが、鍵は両方とも呼び出し時点で判っているので束ねられる。
pub(super) async fn grant_for<R: Reader>(
    txn: &Readers<R>,
    principal: PrincipalId,
    scope: Scope,
) -> Result<Grant, AppError> {
    let key = |target| keys::acl(AclKey::new(principal, target));

    Ok(match scope {
        Scope::Database(db_id) => {
            let found = kv::get(txn, key(DataTarget::database(db_id))).await?;
            Grant::Database(found.as_deref().map(decode_role).transpose()?)
        }
        Scope::Table(db_id, table_id) => {
            let db_key = key(DataTarget::database(db_id));
            let table_key = key(DataTarget::table(db_id, table_id));
            let found = kv::batch_get(txn, vec![db_key.clone(), table_key.clone()]).await?;

            let role_of = |wanted: &[u8]| {
                found
                    .iter()
                    .find(|(key, _)| key == wanted)
                    .map(|(_, value)| decode_role(value))
                    .transpose()
            };
            Grant::Table {
                database: role_of(&db_key)?,
                table: role_of(&table_key)?,
            }
        }
        // 1 行でもあれば足りるので、読むのも 1 行で止める。
        Scope::AnyIn(db_id) => Grant::AnyIn(
            kv::any_key_in_prefix(txn, &keys::acl_owned_by_in(principal, db_id)).await?,
        ),
    })
}

pub(super) async fn acl_entries<R: Reader>(
    txn: &Readers<R>,
    principal: PrincipalId,
) -> Result<Vec<AclEntry>, AppError> {
    kv::scan_prefix(txn, &keys::acl_owned_by(principal))
        .await?
        .iter()
        .map(|(key, value)| {
            Ok(AclEntry {
                target: keys::acl_from_key(key)?.target,
                role: decode_role(value)?,
            })
        })
        .collect()
}

pub(super) async fn acl_databases<R: Reader>(
    txn: &Readers<R>,
    principal: PrincipalId,
) -> Result<BTreeSet<DatabaseId>, AppError> {
    acl_owned(txn, &keys::acl_owned_by(principal))
        .await?
        .into_iter()
        .map(|key| Ok(key.target.db_id))
        .collect()
}

pub(super) async fn acl_tables_in<R: Reader>(
    txn: &Readers<R>,
    principal: PrincipalId,
    db_id: DatabaseId,
) -> Result<BTreeSet<TableId>, AppError> {
    Ok(acl_owned(txn, &keys::acl_owned_by_in(principal, db_id))
        .await?
        .into_iter()
        .filter_map(|key| key.target.table_id)
        .collect())
}

pub(super) async fn acl_count<R: Reader>(
    txn: &Readers<R>,
    principal: PrincipalId,
) -> Result<u32, AppError> {
    Ok(acl_owned(txn, &keys::acl_owned_by(principal)).await?.len() as u32)
}

/// 前向き側の前置に一致する行の鍵。
async fn acl_owned<R: Reader>(txn: &Readers<R>, prefix: &[u8]) -> Result<Vec<AclKey>, AppError> {
    kv::scan_prefix_keys(txn, prefix)
        .await?
        .iter()
        .map(|key| keys::acl_from_key(key))
        .collect()
}

impl<R: Reader> TikvRead<'_, R> {
    #[tracing::instrument(skip_all)]
    pub(super) async fn list_users_impl(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, UserRecord)>, AppError> {
        // `after` の直後から始めるため末尾に 0 を足す（利用者名は空にならない）。
        let start = after.map_or_else(
            || keys::Ns::Users.prefix(),
            |after| {
                let mut key = keys::user(after);
                key.push(0);
                key
            },
        );
        let end = crate::repositories::encoding::prefix_end(&keys::Ns::Users.prefix());

        kv::scan_range_limited(&self.txn, start, end, limit)
            .await?
            .iter()
            .map(|(key, value)| {
                Ok((
                    keys::username_from_key(key)?.to_string(),
                    decode(Stored::UserRecord, value)?,
                ))
            })
            .collect()
    }
}

impl TikvWrite<'_> {
    /// 「存在しないことを確認してから作る」「読んでから書き戻す」を守るための排他。
    pub(super) fn lock_user_impl(&mut self, username: &str) -> Result<(), AppError> {
        self.require_lock(LockScope::User, username.as_bytes())
            .map_err(AppError::from)
    }

    pub(super) async fn put_user_record_impl(
        &mut self,
        username: &str,
        record: &UserRecord,
    ) -> Result<(), AppError> {
        kv::put(
            &self.txn,
            keys::user(username),
            encode(Stored::UserRecord, record)?,
        )
        .await;
        Ok(())
    }

    pub(super) async fn remove_user_record_impl(&mut self, username: &str) -> Result<(), AppError> {
        kv::delete(&self.txn, keys::user(username)).await;
        Ok(())
    }

    pub(super) async fn acl_put_impl(
        &mut self,
        entry: AclEntry,
        principal: PrincipalId,
    ) -> Result<(), AppError> {
        let row = keys::acl_row(AclKey::new(principal, entry.target));
        kv::put_many(
            &self.txn,
            [
                (row.forward, vec![entry.role.into()]),
                (row.reverse, Vec::new()),
            ],
        )
        .await;
        Ok(())
    }

    pub(super) async fn acl_remove_impl(
        &mut self,
        principal: PrincipalId,
        target: DataTarget,
    ) -> Result<bool, AppError> {
        let key = AclKey::new(principal, target);
        let existed = kv::get(&self.txn, keys::acl(key)).await?.is_some();
        self.delete_acl_rows([key]).await;
        Ok(existed)
    }

    /// この対象を指す行を、保持者を問わず落とす。
    ///
    /// 逆引きがデータベース前置なので、`table_id` が `None` のときは
    /// **データベーススコープの行と配下テーブルの行が同じ 1 プレフィックスに入る**。
    pub(super) async fn acl_remove_object_impl(
        &mut self,
        db_id: DatabaseId,
        table_id: Option<TableId>,
    ) -> Result<(), AppError> {
        let prefix = match table_id {
            Some(table_id) => keys::acl_holders_of(DataTarget::table(db_id, table_id)),
            None => keys::acl_holders_in(db_id),
        };
        let victims: Vec<AclKey> = kv::scan_prefix_keys(&self.txn, &prefix)
            .await?
            .iter()
            .map(|key| keys::acl_from_reverse_key(key))
            .collect::<Result<_, _>>()?;

        self.delete_acl_rows(victims).await;
        Ok(())
    }

    pub(super) async fn acl_remove_principal_impl(
        &mut self,
        principal: PrincipalId,
    ) -> Result<(), AppError> {
        let victims = acl_owned(&self.txn, &keys::acl_owned_by(principal)).await?;
        self.delete_acl_rows(victims).await;
        Ok(())
    }

    /// 前向きと逆引きは必ず一組で落とす。片方だけ残ると、前向きにしか見えない行や、
    /// 対象を消しても消えない行ができる。
    async fn delete_acl_rows(&mut self, keys: impl IntoIterator<Item = AclKey>) {
        let doomed: Vec<Vec<u8>> = keys
            .into_iter()
            .flat_map(|key| keys::acl_row(key).into_keys())
            .collect();
        kv::delete_many(&self.txn, doomed).await;
    }
}
