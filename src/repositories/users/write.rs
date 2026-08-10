use crate::{
    error::AppError,
    models::users::{MAX_PRIVILEGE_RULES, PrivilegeRule, PrivilegeTarget, UserMetadata},
    repositories::{KasaneDbWrite, MetaRepository},
};

impl<'a> KasaneDbWrite<'a> {
    fn put_user_meta(&mut self, username: &str, meta: &UserMetadata) -> Result<(), AppError> {
        let json = serde_json::to_string(meta)
            .map_err(|_| AppError::InternalError("Failed to serialize user metadata".into()))?;
        self.db
            .users
            .put(&mut self.write_txn, username, json.as_str())?;
        Ok(())
    }

    /// ユーザーを作成する。`privileges` は名前ベースのまま渡し、同じトランザクション内で
    /// ID へ解決する（存在しないデータベース／テーブルはここで弾かれる）。
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn create_user_impl(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<(), AppError> {
        if MetaRepository::user_meta(self, username).await?.is_some() {
            return Err(AppError::Conflict("User already exists".to_string()));
        }
        let meta = UserMetadata {
            id,
            password_hash,
            token_version: 0,
            privileges: self.resolve_privileges(privileges).await?,
        };
        self.put_user_meta(username, &meta)
    }

    /// パスワードを差し替え、`token_version` を進めて発行済みトークンを失効させる。
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn set_password_impl(
        &mut self,
        username: &str,
        password_hash: String,
    ) -> Result<(), AppError> {
        let mut meta = self.require_user_meta(username).await?;
        meta.password_hash = password_hash;
        meta.token_version = meta.token_version.wrapping_add(1);
        self.put_user_meta(username, &meta)
    }

    /// 1 つの対象に対する権限を設定する（無ければ追加、あれば置き換え）。
    ///
    /// 触るのはその対象 1 件だけなので、別の対象に対する同時の付与・剥奪と干渉しない。
    /// 読み出しから書き込みまでこの 1 つの書き込みトランザクション内で完結する。
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn grant_privilege_impl(
        &mut self,
        username: &str,
        rule: &PrivilegeRule,
    ) -> Result<(), AppError> {
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
        self.put_user_meta(username, &meta)
    }

    /// 1 つの対象に対する権限を剥奪する。ロールは問わず対象ごと落とす。
    /// 該当するルールが無ければ `NotFound`。
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn revoke_privilege_impl(
        &mut self,
        username: &str,
        target: &PrivilegeTarget,
    ) -> Result<(), AppError> {
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

        self.put_user_meta(username, &meta)
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn delete_user_impl(&mut self, username: &str) -> Result<(), AppError> {
        self.require_user_meta(username).await?;
        self.db.users.delete(&mut self.write_txn, username)?;
        Ok(())
    }
}
