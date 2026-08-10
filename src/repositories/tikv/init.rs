//! TiKV 固有の初期化設定。
//!
//! 接続先の解決・クライアントの構築・既定ユーザーの投入といった
//! 「起動時に一度だけ行うこと」をここに集める。定常運用の読み書きは `mod.rs` 以下が担う。

use std::sync::Arc;

use tikv_client::TransactionClient;

use crate::error::AppError;
use crate::models::users::{PrivilegeRule, UserRole};
use crate::repositories::{CatalogRepository, Storage, WriteRepository};
use crate::services::auth::hash_password;

use super::{TikvDb, to_app_error};

/// PD エンドポイントの既定値。`KASANE_TIKV_PD_ENDPOINTS` で上書きできる。
const DEFAULT_PD_ENDPOINTS: &str = "127.0.0.1:2379";

/// 初期投入する管理者ユーザーの名前。
const ROOT_USERNAME: &str = "root";

/// TiKV への接続設定。
#[derive(Debug, Clone)]
pub struct TikvConfig {
    /// PD のエンドポイント（`host:port`）。
    pub pd_endpoints: Vec<String>,
}

impl TikvConfig {
    /// 環境変数から組み立てる。`KASANE_TIKV_PD_ENDPOINTS` はカンマ区切り。
    pub fn from_env() -> Self {
        let raw = std::env::var("KASANE_TIKV_PD_ENDPOINTS")
            .unwrap_or_else(|_| DEFAULT_PD_ENDPOINTS.to_string());
        Self {
            pd_endpoints: raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }
}

impl TikvDb {
    /// クラスタへ接続し、既定ユーザーを用意して返す。
    pub async fn connect(config: TikvConfig) -> Result<Self, AppError> {
        let client = TransactionClient::new(config.pd_endpoints.clone())
            .await
            .map_err(to_app_error)?;
        tracing::info!("connected to TiKV via PD {:?}", config.pd_endpoints);
        let db = Self {
            client: Arc::new(client),
        };
        db.ensure_root_user().await?;
        Ok(db)
    }

    /// 既定の root ユーザーが無ければ作る（LMDB の初期化と同じ役割）。
    ///
    /// クラスタは複数の Kasane インスタンスで共有されうるので、同時に起動しても
    /// 二重作成にならないよう通常の書き込み経路を通す。ユーザー単位のロックで
    /// 直列化され、後から来た側は既に存在することを見て何もしない。
    async fn ensure_root_user(&self) -> Result<(), AppError> {
        // 既にあるなら、パスワードのハッシュ化コストを払わずに抜ける。
        let existing = self
            .read(async |r| CatalogRepository::user_meta(r, ROOT_USERNAME).await)
            .await?;
        if existing.is_some() {
            return Ok(());
        }

        let password = std::env::var("ROOT_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "password".to_string());
        let hash = hash_password(&password)?;
        let id = uuid::Uuid::now_v7();

        tracing::info!("Creating default root user: {ROOT_USERNAME}");
        let created = self
            .write(async move |w| {
                match w
                    .create_user(
                        ROOT_USERNAME,
                        id,
                        hash.clone(),
                        &[PrivilegeRule::Global {
                            role: UserRole::Admin,
                        }],
                    )
                    .await
                {
                    // 別インスタンスが先に作っていた場合は、それで目的は果たされている。
                    Err(AppError::Conflict(_)) => Ok(false),
                    other => other.map(|_| true),
                }
            })
            .await?;

        if !created {
            tracing::info!("root user already created by another instance");
        }
        Ok(())
    }
}
