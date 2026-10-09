//! 利用者レコードと ACL 行の読み書き。
//!
//! 名前 ⇄ ID 変換と付与・剥奪の意味論は
//! [`CatalogRepository`](crate::repositories::CatalogRepository) /
//! [`WriteRepository`](crate::repositories::WriteRepository) の既定実装が持つ。
//! ここにあるのは行を読み書きする手段だけ。

use std::collections::BTreeSet;
use std::ops::Bound;

use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{AclEntry, DataRole, Grant, Scope, UserRecord};

use super::State;

impl State {
    pub(super) fn user_record_impl(&self, username: &str) -> Option<UserRecord> {
        self.users.get(username).cloned()
    }

    /// 利用者名の辞書順で、`after` より後ろから最大 `limit` 件。
    pub(super) fn list_users_impl(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Vec<(String, UserRecord)> {
        let lower = after.map_or(Bound::Unbounded, Bound::Excluded);
        self.users
            .range::<str, _>((lower, Bound::Unbounded))
            .take(limit)
            .map(|(name, record)| (name.clone(), record.clone()))
            .collect()
    }

    pub(super) fn put_user_record_impl(&mut self, username: &str, record: &UserRecord) {
        self.users.insert(username.to_string(), record.clone());
    }

    pub(super) fn remove_user_record_impl(&mut self, username: &str) {
        self.users.remove(username);
    }

    /// スコープの判定に必要な行だけを読む。
    pub(super) fn grant_for_impl(&self, principal: PrincipalId, scope: Scope) -> Grant {
        match scope {
            Scope::Database(db_id) => {
                Grant::Database(self.acl_role(principal, DataTarget::database(db_id)))
            }
            Scope::Table(db_id, table_id) => Grant::Table {
                database: self.acl_role(principal, DataTarget::database(db_id)),
                table: self.acl_role(principal, DataTarget::table(db_id, table_id)),
            },
            Scope::AnyIn(db_id) => Grant::AnyIn(
                self.acl_of(principal)
                    .any(|(target, _)| target.db_id == db_id),
            ),
        }
    }

    pub(super) fn acl_entries_impl(&self, principal: PrincipalId) -> Vec<AclEntry> {
        self.acl_of(principal)
            .map(|(target, role)| AclEntry { target, role })
            .collect()
    }

    pub(super) fn acl_databases_impl(&self, principal: PrincipalId) -> BTreeSet<DatabaseId> {
        self.acl_of(principal)
            .map(|(target, _)| target.db_id)
            .collect()
    }

    pub(super) fn acl_tables_in_impl(
        &self,
        principal: PrincipalId,
        db_id: DatabaseId,
    ) -> BTreeSet<TableId> {
        self.acl_of(principal)
            .filter(|(target, _)| target.db_id == db_id)
            .filter_map(|(target, _)| target.table_id)
            .collect()
    }

    pub(super) fn acl_count_impl(&self, principal: PrincipalId) -> u32 {
        self.acl_of(principal).count() as u32
    }

    pub(super) fn acl_put_impl(&mut self, entry: AclEntry, principal: PrincipalId) {
        self.acl.insert((principal, entry.target), entry.role);
    }

    /// 戻り値は行があったか。
    pub(super) fn acl_remove_impl(&mut self, principal: PrincipalId, target: DataTarget) -> bool {
        self.acl.remove(&(principal, target)).is_some()
    }

    pub(super) fn acl_remove_principal_impl(&mut self, principal: PrincipalId) {
        self.acl.retain(|(owner, _), _| *owner != principal);
    }

    /// この対象を指す行を、保持者を問わず落とす。`table_id` が `None` なら配下のテーブルの行も落とす。
    pub(super) fn acl_remove_object_impl(&mut self, db_id: DatabaseId, table_id: Option<TableId>) {
        self.acl.retain(|(_, target), _| match table_id {
            Some(table_id) => *target != DataTarget::table(db_id, table_id),
            None => target.db_id != db_id,
        });
    }

    fn acl_of(&self, principal: PrincipalId) -> impl Iterator<Item = (DataTarget, DataRole)> {
        self.acl
            .iter()
            .filter(move |((owner, _), _)| *owner == principal)
            .map(|((_, target), role)| (*target, *role))
    }

    fn acl_role(&self, principal: PrincipalId, target: DataTarget) -> Option<DataRole> {
        self.acl.get(&(principal, target)).copied()
    }
}
