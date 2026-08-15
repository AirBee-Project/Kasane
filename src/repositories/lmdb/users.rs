//! 利用者レコードと ACL 行の永続化。
//!
//! 名前 ⇄ ID 変換と付与・剥奪の意味論は
//! [`CatalogRepository`](crate::repositories::CatalogRepository) /
//! [`WriteRepository`](crate::repositories::WriteRepository) の既定実装が持つ。
//! ここにあるのは行を読み書きする手段だけ。
//!
//! 読み取りは [`AppDb`] のメソッドにしてある。`RwTxn` は `RoTxn` へ Deref するので、
//! 読み書きどちらのトランザクションからも同じ実装を通せる。

use std::collections::BTreeSet;

use heed::RoTxn;

use crate::error::{AppError, Stored};
use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{AclEntry, Grant, Scope, UserRecord};
use crate::repositories::encoding::acl::{AclKey, decode_role};

use super::{AppDb, KasaneDbRead, KasaneDbWrite};

type Txn<'t> = RoTxn<'t, heed::WithoutTls>;

impl AppDb {
    pub(super) fn user_record(
        &self,
        txn: &Txn<'_>,
        username: &str,
    ) -> Result<Option<UserRecord>, AppError> {
        self.users
            .get(txn, username)?
            .map(|json| {
                serde_json::from_str(json).map_err(|e| AppError::corrupt(Stored::UserRecord, e))
            })
            .transpose()
    }

    fn acl_role(
        &self,
        txn: &Txn<'_>,
        principal: PrincipalId,
        target: DataTarget,
    ) -> Result<Option<crate::models::users::DataRole>, AppError> {
        self.acl
            .get(txn, &AclKey::new(principal, target).forward())?
            .map(decode_role)
            .transpose()
    }

    /// スコープの判定に必要な行だけを読む。
    pub(super) fn grant_for(
        &self,
        txn: &Txn<'_>,
        principal: PrincipalId,
        scope: Scope,
    ) -> Result<Grant, AppError> {
        Ok(match scope {
            Scope::Database(db_id) => {
                Grant::Database(self.acl_role(txn, principal, DataTarget::database(db_id))?)
            }
            Scope::Table(db_id, table_id) => Grant::Table {
                database: self.acl_role(txn, principal, DataTarget::database(db_id))?,
                table: self.acl_role(txn, principal, DataTarget::table(db_id, table_id))?,
            },
            // 1 行でもあれば足りるので、読むのも 1 行で止める。
            Scope::AnyIn(db_id) => Grant::AnyIn(
                self.acl
                    .prefix_iter(txn, &AclKey::owned_by_in(principal, db_id))?
                    .next()
                    .transpose()?
                    .is_some(),
            ),
        })
    }

    pub(super) fn acl_entries(
        &self,
        txn: &Txn<'_>,
        principal: PrincipalId,
    ) -> Result<Vec<AclEntry>, AppError> {
        let mut out = Vec::new();
        for item in self.acl.prefix_iter(txn, &AclKey::owned_by(principal))? {
            let (key, value) = item?;
            out.push(AclEntry {
                target: AclKey::decode_forward(key)?.target,
                role: decode_role(value)?,
            });
        }
        Ok(out)
    }

    pub(super) fn acl_databases(
        &self,
        txn: &Txn<'_>,
        principal: PrincipalId,
    ) -> Result<BTreeSet<DatabaseId>, AppError> {
        self.acl_owned(txn, &AclKey::owned_by(principal))?
            .into_iter()
            .map(|key| Ok(key.target.db_id))
            .collect()
    }

    pub(super) fn acl_tables_in(
        &self,
        txn: &Txn<'_>,
        principal: PrincipalId,
        db_id: DatabaseId,
    ) -> Result<BTreeSet<TableId>, AppError> {
        Ok(self
            .acl_owned(txn, &AclKey::owned_by_in(principal, db_id))?
            .into_iter()
            .filter_map(|key| key.target.table_id)
            .collect())
    }

    pub(super) fn acl_count(&self, txn: &Txn<'_>, principal: PrincipalId) -> Result<u32, AppError> {
        Ok(self.acl_owned(txn, &AclKey::owned_by(principal))?.len() as u32)
    }

    /// 前向き側の前置に一致する行の鍵。
    fn acl_owned(&self, txn: &Txn<'_>, prefix: &[u8]) -> Result<Vec<AclKey>, AppError> {
        let mut out = Vec::new();
        for item in self.acl.prefix_iter(txn, prefix)? {
            out.push(AclKey::decode_forward(item?.0)?);
        }
        Ok(out)
    }

    /// 逆引き側の前置に一致する行の鍵。
    fn acl_holders(&self, txn: &Txn<'_>, prefix: &[u8]) -> Result<Vec<AclKey>, AppError> {
        let mut out = Vec::new();
        for item in self.acl_by_object.prefix_iter(txn, prefix)? {
            out.push(AclKey::decode_reverse(item?.0)?);
        }
        Ok(out)
    }
}

impl KasaneDbRead<'_> {
    #[tracing::instrument(skip_all)]
    pub fn list_users_impl(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, UserRecord)>, AppError> {
        use std::ops::Bound;

        // 名前の辞書順で進む。`Excluded` なので `after` 自身は含まれない。
        let lower = after.map_or(Bound::Unbounded, Bound::Excluded);

        self.db
            .users
            .range(&self.read_txn, &(lower, Bound::Unbounded))?
            .take(limit)
            .map(|item| {
                let (username, json) = item?;
                let record = serde_json::from_str(json)
                    .map_err(|e| AppError::corrupt(Stored::UserRecord, e))?;
                Ok((username.to_string(), record))
            })
            .collect()
    }
}

impl KasaneDbWrite<'_> {
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub(super) fn put_user_record_impl(
        &mut self,
        username: &str,
        record: &UserRecord,
    ) -> Result<(), AppError> {
        let json =
            serde_json::to_string(record).map_err(|e| AppError::corrupt(Stored::UserRecord, e))?;
        self.db
            .users
            .put(&mut self.write_txn, username, json.as_str())?;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub(super) fn remove_user_record_impl(&mut self, username: &str) -> Result<(), AppError> {
        self.db.users.delete(&mut self.write_txn, username)?;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(principal = %principal))]
    pub(super) fn acl_put_impl(
        &mut self,
        entry: AclEntry,
        principal: PrincipalId,
    ) -> Result<(), AppError> {
        let row = AclKey::new(principal, entry.target).rows();
        self.db
            .acl
            .put(&mut self.write_txn, &row.forward, &[entry.role.into()])?;
        self.db
            .acl_by_object
            .put(&mut self.write_txn, &row.reverse, &())?;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(principal = %principal))]
    pub(super) fn acl_remove_impl(
        &mut self,
        principal: PrincipalId,
        target: DataTarget,
    ) -> Result<bool, AppError> {
        let key = AclKey::new(principal, target);
        let existed = self.db.acl.get(&self.write_txn, &key.forward())?.is_some();
        self.delete_acl_rows(&[key])?;
        Ok(existed)
    }

    /// この対象を指す行を、保持者を問わず落とす。
    ///
    /// 逆引きがデータベース前置なので、`target` がデータベースを指すときは
    /// **データベーススコープの行と配下テーブルの行が同じ 1 プレフィックスに入る**。
    #[tracing::instrument(skip_all, fields(db_id = %db_id, table_id = ?table_id))]
    pub(super) fn acl_remove_object_impl(
        &mut self,
        db_id: DatabaseId,
        table_id: Option<TableId>,
    ) -> Result<(), AppError> {
        let prefix = match table_id {
            Some(table_id) => AclKey::holders_of(DataTarget::table(db_id, table_id)),
            None => AclKey::holders_in(db_id),
        };
        // 反復中に削除できないので、まず鍵を集める。
        let victims = self.db.acl_holders(&self.write_txn, &prefix)?;
        self.delete_acl_rows(&victims)
    }

    #[tracing::instrument(skip_all, fields(principal = %principal))]
    pub(super) fn acl_remove_principal_impl(
        &mut self,
        principal: PrincipalId,
    ) -> Result<(), AppError> {
        let victims = self
            .db
            .acl_owned(&self.write_txn, &AclKey::owned_by(principal))?;
        self.delete_acl_rows(&victims)
    }

    /// 前向きと逆引きは必ず一組で落とす。片方だけ残ると、前向きにしか見えない行や、
    /// 対象を消しても消えない行ができる。
    fn delete_acl_rows(&mut self, keys: &[AclKey]) -> Result<(), AppError> {
        for key in keys {
            let row = key.rows();
            self.db.acl.delete(&mut self.write_txn, &row.forward)?;
            self.db
                .acl_by_object
                .delete(&mut self.write_txn, &row.reverse)?;
        }
        Ok(())
    }
}
