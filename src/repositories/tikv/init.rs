//! クラスタへの接続設定と初期化。

use super::keys::{self, LockScope};
use super::kv::{self, Reader};
use super::{TikvDb, TikvRead, TikvWrite};
use crate::error::AppError;
use crate::models::users::{PrivilegeRule, UserRole};
use crate::repositories::{CatalogRepository, Storage, WriteRepository};
use crate::services::auth::hash_password;
use std::path::PathBuf;
use std::sync::Arc;
use tikv_client::{Config, TransactionClient};

pub(super) const DEFAULT_PD_ENDPOINTS: &str = "127.0.0.1:2379";

const ROOT_USERNAME: &str = "root";

/// tikv-client の既定 2 秒は短すぎる。これは**すべての**ストア RPC の期限になるが、
/// 混み合ったノードでは 1024 キーの `batch_get` も数千キーの prewrite も普通に超える。
/// 超えた瞬間 tonic がストリームを RST で打ち切るので、**遅いだけで済んだはずの要求が
/// 失敗に化け、やり直しでさらに混雑を増やす**。同じクライアントがロックへ与える寿命は
/// 20 秒なので、その近辺まで伸ばして詰まりの検出は HTTP/2 の keep-alive に任せる。
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;

/// TiKV への接続設定。
#[derive(Debug, Clone)]
pub struct TikvConfig {
    pub pd_endpoints: Vec<String>,
    pub security: Option<TikvSecurity>,
    /// 既定は `DEFAULT_REQUEST_TIMEOUT_SECS`。
    pub request_timeout: std::time::Duration,
}

#[derive(Debug, Clone)]
pub struct TikvSecurity {
    pub ca_path: PathBuf,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl TikvConfig {
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
            request_timeout: request_timeout_from_env(),
        }
    }
}

/// 未設定・解析不能なら既定値（LMDB 側の同名関数と対）。
pub(super) fn env_parsed<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(default)
}

/// `0` は「待たない」ではなく設定ミスなので既定へ落とす。
fn request_timeout_from_env() -> std::time::Duration {
    let secs = match env_parsed(
        "KASANE_TIKV_REQUEST_TIMEOUT_SECS",
        DEFAULT_REQUEST_TIMEOUT_SECS,
    ) {
        0 => DEFAULT_REQUEST_TIMEOUT_SECS,
        secs => secs,
    };
    std::time::Duration::from_secs(secs)
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
    #[tracing::instrument(skip_all, fields(pd_endpoints = ?config.pd_endpoints))]
    pub async fn connect(config: TikvConfig) -> Result<Self, AppError> {
        let mut client_config = Config::default().with_timeout(config.request_timeout);
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

        let client =
            TransactionClient::new_with_config(config.pd_endpoints.clone(), client_config).await?;
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
        if self.read(async |r| r.cluster_initialized().await).await? {
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
}

/// 存在するかどうかだけを見るので、値に意味は無い。
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
        .await;
        Ok(!already_seeded)
    }
}
