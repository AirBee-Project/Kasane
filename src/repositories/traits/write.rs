//! 書き込みトランザクション上で行える操作。

use spatial_id::collection::SpatialIdSet;

use crate::error::{AppError, Resource};
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{Table, TableConstraints, TableDataType};
use crate::models::id::{DataTarget, PrincipalId, TableId};
use crate::models::users::{
    AclEntry, MAX_PRIVILEGES_PER_USER, PrivilegeRule, PrivilegeTarget, ResolvedPrivilege,
    ResolvedTarget, UserRecord,
};

use super::CatalogRepository;

// `async fn` の Future に `Send` を課さない理由は [`Storage`](super::Storage) を参照。
#[allow(async_fn_in_trait)]
pub trait WriteRepository: CatalogRepository {
    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError>;

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError>;

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<DatabaseInfoResponse, AppError>;

    #[allow(clippy::too_many_arguments)]
    async fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        is_temporal: bool,
    ) -> Result<Table, AppError>;

    /// `is_temporal`: `Some(true)` で時間ロックを解除する。`Some(false)`（再ロック）は
    /// 既存データに時間成分が残っていないかを定数時間で確認する手段が無いため未対応で、サービスレイヤーが事前に弾く。
    #[allow(clippy::too_many_arguments)]
    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<TableConstraints>>,
        description: Option<Option<String>>,
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError>;

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError>;

    async fn data_insert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    async fn data_remove(&mut self, table_id: TableId, ids: SpatialIdSet) -> Result<(), AppError>;

    // --- 利用者と ACL の素の操作 ---
    //
    // 実装が用意するのはここまで。付与・剥奪の意味論は下の既定実装が持つ。

    async fn put_user_record(
        &mut self,
        username: &str,
        record: &UserRecord,
    ) -> Result<(), AppError>;

    async fn remove_user_record(&mut self, username: &str) -> Result<(), AppError>;

    /// 1 行を書く（既にあれば置き換え）。
    async fn acl_put(&mut self, entry: AclEntry, principal: PrincipalId) -> Result<(), AppError>;

    /// 1 行を消す。戻り値は**あったか**（剥奪が 404 を返すかの判断に要る）。
    async fn acl_remove(
        &mut self,
        principal: PrincipalId,
        target: DataTarget,
    ) -> Result<bool, AppError>;

    /// この主体の行をすべて消す。利用者の削除で使う。
    async fn acl_remove_principal(&mut self, principal: PrincipalId) -> Result<(), AppError>;

    /// この主体が持っている行数。上限の判定に使う。
    async fn acl_count(&self, principal: PrincipalId) -> Result<u32, AppError>;

    // --- 既定実装 ---

    /// `privileges` を名前のまま受け取り同じトランザクション内で解決するので、存在しない
    /// データベース／テーブルはここで弾かれる。
    async fn create_user(
        &mut self,
        username: &str,
        id: PrincipalId,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<(), AppError> {
        if self.user_record(username).await?.is_some() {
            return Err(Resource::User.already_exists(username));
        }

        let mut record = UserRecord {
            id,
            password_hash,
            token_version: 0,
            global_role: None,
        };

        for resolved in self.resolve_rules(privileges).await? {
            match resolved {
                ResolvedPrivilege::Global(role) => record.global_role = Some(role),
                ResolvedPrivilege::Data { target, role } => {
                    self.acl_put(AclEntry { target, role }, id).await?;
                }
            }
        }

        self.put_user_record(username, &record).await
    }

    /// `token_version` を進めて発行済みトークンを失効させる。
    async fn set_password(
        &mut self,
        username: &str,
        password_hash: String,
    ) -> Result<(), AppError> {
        let mut record = self.require_user_record(username).await?;
        record.password_hash = password_hash;
        record.token_version = record.token_version.wrapping_add(1);
        self.put_user_record(username, &record).await
    }

    /// 1 つの対象に対する権限を設定する（無ければ追加、あれば置き換え）。
    ///
    /// 触るのは 1 行だけなので、別の対象への同時の付与・剥奪とは干渉しない。
    async fn grant_privilege(
        &mut self,
        username: &str,
        rule: &PrivilegeRule,
    ) -> Result<(), AppError> {
        let resolved = self.resolve_rule(rule).await?;
        let record = self.require_user_record(username).await?;

        let (target, role) = match resolved {
            ResolvedPrivilege::Global(role) => {
                return self
                    .put_user_record(
                        username,
                        &UserRecord {
                            global_role: Some(role),
                            ..record
                        },
                    )
                    .await;
            }
            ResolvedPrivilege::Data { target, role } => (target, role),
        };

        if self.acl_count(record.id).await? >= MAX_PRIVILEGES_PER_USER {
            return Err(AppError::too_many_privileges());
        }
        self.acl_put(AclEntry { target, role }, record.id).await
    }

    /// 1 つの対象に対する権限を剥奪する。ロールは問わず対象ごと落とす。
    async fn revoke_privilege(
        &mut self,
        username: &str,
        target: &PrivilegeTarget,
    ) -> Result<(), AppError> {
        let not_found = || {
            Resource::Privilege.not_found(match target {
                PrivilegeTarget::Global => "global".to_string(),
                PrivilegeTarget::Database { db_name } => db_name.clone(),
                PrivilegeTarget::Table {
                    db_name,
                    table_name,
                } => format!("{db_name}.{table_name}"),
            })
        };
        let resolved = self.resolve_target(target).await?;
        let mut record = self.require_user_record(username).await?;

        match resolved {
            ResolvedTarget::Global => {
                if record.global_role.take().is_none() {
                    return Err(not_found());
                }
                self.put_user_record(username, &record).await
            }
            ResolvedTarget::Data(target) => {
                if self.acl_remove(record.id, target).await? {
                    Ok(())
                } else {
                    Err(not_found())
                }
            }
        }
    }

    async fn delete_user(&mut self, username: &str) -> Result<(), AppError> {
        let record = self.require_user_record(username).await?;
        self.acl_remove_principal(record.id).await?;
        self.remove_user_record(username).await
    }
}
