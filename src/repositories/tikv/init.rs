use super::keys::{self, LockScope};
use super::kv::{self, Reader};
use super::{TikvDb, TikvRead, TikvWrite, to_app_error};
use crate::error::AppError;
use crate::models::users::{PrivilegeRule, UserRole};
use crate::repositories::{CatalogRepository, Storage, WriteRepository};
use crate::services::auth::hash_password;
use std::path::PathBuf;
use std::sync::Arc;
use tikv_client::{Config, TransactionClient};

/// PD エンドポイントの既定値
pub(super) const DEFAULT_PD_ENDPOINTS: &str = "127.0.0.1:2379";

/// 初期投入する管理者ユーザーの名前
const ROOT_USERNAME: &str = "root";

/// TiKV への接続設定。
#[derive(Debug, Clone)]
pub struct TikvConfig {
    pub pd_endpoints: Vec<String>,
    pub security: Option<TikvSecurity>,
}

#[derive(Debug, Clone)]
pub struct TikvSecurity {
    pub ca_path: PathBuf,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl TikvConfig {
    /// 環境変数から組み立てる
    pub fn from_env() -> Self {
        Self::from_endpoints(
            &std::env::var("KASANE_TIKV_PD_ENDPOINTS")
                .unwrap_or_else(|_| DEFAULT_PD_ENDPOINTS.to_string()),
        )
    }

    pub fn from_endpoints(raw: &str) -> Self {
        Self {
            pd_endpoints: raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            security: TikvSecurity::from_env(),
        }
    }
}

impl TikvSecurity {
    fn from_env() -> Option<Self> {
        let path = |name: &str| std::env::var(name).ok().filter(|s| !s.is_empty());
        match (
            path("KASANE_TIKV_CA_PATH"),
            path("KASANE_TIKV_CERT_PATH"),
            path("KASANE_TIKV_KEY_PATH"),
        ) {
            (Some(ca), Some(cert), Some(key)) => Some(Self {
                ca_path: ca.into(),
                cert_path: cert.into(),
                key_path: key.into(),
            }),
            (None, None, None) => None,
            _ => {
                tracing::warn!("TiKV の TLS 設定が不完全です。");
                None
            }
        }
    }
}

impl TikvDb {
    /// クラスタへ接続し、既定ユーザーを用意して返す。
    pub async fn connect(config: TikvConfig) -> Result<Self, AppError> {
        let mut client_config = Config::default();
        match &config.security {
            Some(sec) => {
                client_config = client_config.with_security(
                    sec.ca_path.clone(),
                    sec.cert_path.clone(),
                    sec.key_path.clone(),
                );
            }
            None => tracing::warn!(
                "connecting to TiKV without TLS; anyone able to reach PD/TiKV can read and write \
                 all data. set KASANE_TIKV_CA_PATH / KASANE_TIKV_CERT_PATH / KASANE_TIKV_KEY_PATH \
                 unless the network path is already closed"
            ),
        }

        let client = TransactionClient::new_with_config(config.pd_endpoints.clone(), client_config)
            .await
            .map_err(to_app_error)?;
        tracing::info!(
            "connected to TiKV via PD {:?} (tls: {})",
            config.pd_endpoints,
            config.security.is_some()
        );
        let db = Self {
            client: Arc::new(client),
        };
        db.ensure_initialized().await?;
        Ok(db)
    }

    async fn ensure_initialized(&self) -> Result<(), AppError> {
        if self.is_initialized().await? {
            return Ok(());
        }

        let password = std::env::var("ROOT_PASSWORD")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "password".to_string());
        let hash = hash_password(&password)?;
        let id = uuid::Uuid::now_v7();

        let created = self
            .write(async move |w| {
                w.initialize_cluster(
                    ROOT_USERNAME,
                    id,
                    hash.clone(),
                    &[PrivilegeRule::Global {
                        role: UserRole::Admin,
                    }],
                )
                .await
            })
            .await?;

        if created {
            tracing::info!("initialized the cluster and created the default {ROOT_USERNAME} user");
        } else {
            tracing::info!("cluster was already initialized by another instance");
        }
        Ok(())
    }

    async fn is_initialized(&self) -> Result<bool, AppError> {
        self.read(async |r| r.cluster_initialized().await).await
    }
}

/// マーカーの値そのものに意味は無いので、存在するかどうかだけを見る。
const MARKER_PRESENT: &[u8] = b"1";

impl<R: Reader> TikvRead<'_, R> {
    pub(super) async fn cluster_initialized(&self) -> Result<bool, AppError> {
        Ok(kv::get(&self.txn, keys::cluster_initialized())
            .await?
            .is_some())
    }
}

impl TikvWrite<'_> {
    async fn initialize_cluster(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<bool, AppError> {
        self.require_lock(LockScope::User, username.as_bytes())?;

        if kv::get(&self.txn, keys::cluster_initialized())
            .await?
            .is_some()
        {
            return Ok(false);
        }

        let already_seeded = self.user_meta(username).await?.is_some();
        if !already_seeded {
            self.create_user(username, id, password_hash, privileges)
                .await?;
        }

        kv::put(
            &self.txn,
            keys::cluster_initialized(),
            MARKER_PRESENT.to_vec(),
        )
        .await?;
        Ok(!already_seeded)
    }
}
