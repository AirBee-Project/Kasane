//! 書き込みトランザクション上で行える操作。

use kasane_logic::SpatialIdSet;

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, UpdateTableConstraints,
};
use crate::models::id::{DataTarget, PrincipalId, TableId};
use crate::models::users::{
    AclEntry, MAX_PRIVILEGES_PER_USER, PrivilegeRule, PrivilegeTarget, ResolvedPrivilege,
    ResolvedTarget, UserRecord,
};

use super::CatalogRepository;

// `async fn` の Future に `Send` を課さない理由は [`Storage`](super::Storage) を参照。
#[allow(async_fn_in_trait)]
pub trait WriteRepository: CatalogRepository {
    // 作成前の重複確認のため「同じトランザクション内で読んでから書く」経路も要る。

    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError>;

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError>;

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
    ) -> Result<(), AppError>;

    async fn database_copy(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError>;

    /// `value_index`: 値インデックスを維持するか。作成後は変更できない。
    #[allow(clippy::too_many_arguments)]
    async fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        value_index: bool,
    ) -> Result<Table, AppError>;

    #[allow(clippy::too_many_arguments)]
    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
        validate_existing_data: bool,
    ) -> Result<Table, AppError>;

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError>;

    async fn table_copy(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError>;

    /// `index` は値インデックスへ反映する型。`None` なら索引を維持しない。
    ///
    /// 「索引するか」と「どう索引するか」を 1 引数にまとめてあるのは、書き込み経路が型を
    /// 必要とするのが索引キーの順序保存エンコードだけだから。
    async fn data_insert(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    /// `entries` は `(空間 ID, 値)` の並び。同じ空間 ID が複数回現れたら後勝ち。
    ///
    /// 1 件ずつ [`data_insert`](Self::data_insert) を呼ぶのと結果は同じだが、シャードを
    /// 1 度しか読み書きしない。このツリーは 1 件の変更でもリーフを丸ごと書き直すので、
    /// 別々に書くと「リーフのサイズ × 件数」を書くことになる。
    async fn data_insert_many(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        entries: Vec<(SpatialIdSet, Vec<u8>)>,
    ) -> Result<(), AppError>;

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError>;

    async fn data_remove(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
    ) -> Result<(), AppError>;

    // --- 利用者と ACL の素の操作 ---
    //
    // 実装が用意するのはここまで。付与・剥奪の意味論は下の既定実装が持つ。

    /// この利用者を触ることを宣言する。
    ///
    /// 単一ライタのバックエンドでは何もしない。分散バックエンドでは
    /// 「存在しないことを確認してから作る」「読んでから書き戻す」を守るための排他になる。
    async fn lock_user(&mut self, username: &str) -> Result<(), AppError>;

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
        self.lock_user(username).await?;

        if self.user_record(username).await?.is_some() {
            return Err(AppError::Conflict("User already exists".to_string()));
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
        self.lock_user(username).await?;

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
        self.lock_user(username).await?;

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
        self.lock_user(username).await?;

        let target = self.resolve_target(target).await?;
        let mut record = self.require_user_record(username).await?;

        match target {
            ResolvedTarget::Global => {
                if record.global_role.take().is_none() {
                    return Err(AppError::no_such_privilege());
                }
                self.put_user_record(username, &record).await
            }
            ResolvedTarget::Data(target) => {
                if self.acl_remove(record.id, target).await? {
                    Ok(())
                } else {
                    Err(AppError::no_such_privilege())
                }
            }
        }
    }

    async fn delete_user(&mut self, username: &str) -> Result<(), AppError> {
        self.lock_user(username).await?;

        let record = self.require_user_record(username).await?;
        self.acl_remove_principal(record.id).await?;
        self.remove_user_record(username).await
    }
}
