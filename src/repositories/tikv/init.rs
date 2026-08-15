//! クラスタへの接続設定と初期化。

use super::keys::{self, LockScope};
use super::kv::{self, Reader};
use super::{TikvDb, TikvRead, TikvWrite};
use crate::error::AppError;
use crate::models::id::PrincipalId;
use crate::models::users::{PrivilegeRule, UserRole};
use crate::repositories::{
    CatalogRepository, ROOT_USERNAME, SCHEMA_VERSION, Storage, WriteRepository, root_password,
};
use crate::services::auth::hash_password;
use std::path::PathBuf;
use std::sync::Arc;
use tikv_client::{Config, TransactionClient};

pub(super) const DEFAULT_PD_ENDPOINTS: &str = "127.0.0.1:2379";

/// tikv-client の既定 2 秒は短すぎる。これは**すべての**ストア RPC の期限になるが、
/// 混み合ったノードでは 1024 キーの `batch_get` も数千キーの prewrite も普通に超える。
/// 超えた瞬間 tonic がストリームを RST で打ち切るので、**遅いだけで済んだはずの要求が
/// 失敗に化け、やり直しでさらに混雑を増やす**。同じクライアントがロックへ与える寿命は
/// 20 秒なので、その近辺まで伸ばして詰まりの検出は HTTP/2 の keep-alive に任せる。
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;

/// tikv-client 既定の 4MiB はシャード側の上限（[`MAX_SHARD_BYTES`]）を意識したものではない。
/// 内部のバッチ分割で 1 RPC あたりのバイト量は既に `SHARD_BATCH_BYTE_BUDGET` 前後に
/// 収めているが、gRPC 側の受信上限はそれとは独立な多重防御として、余裕を持たせた値へ
/// 引き上げておく。
///
/// [`MAX_SHARD_BYTES`]: crate::repositories::encoding::shard_entry::MAX_SHARD_BYTES
const DEFAULT_GRPC_MAX_DECODING_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// TiKV への接続設定。
#[derive(Debug, Clone)]
pub struct TikvConfig {
    pub pd_endpoints: Vec<String>,
    pub security: Option<TikvSecurity>,
    /// 既定は `DEFAULT_REQUEST_TIMEOUT_SECS`。
    pub request_timeout: std::time::Duration,
    /// 既定は `DEFAULT_GRPC_MAX_DECODING_MESSAGE_SIZE`。
    pub grpc_max_decoding_message_size: usize,
}

#[derive(Debug, Clone)]
pub struct TikvSecurity {
    pub ca_path: PathBuf,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl TikvConfig {
    pub fn from_env() -> Self {
        Self::from_endpoints(&env_str("KASANE_TIKV_PD_ENDPOINTS").unwrap_or_default())
    }

    /// `raw` はカンマ区切り。空白は落とし、空なら既定の 1 台へ繋ぐ。
    pub fn from_endpoints(raw: &str) -> Self {
        let mut pd_endpoints: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if pd_endpoints.is_empty() {
            pd_endpoints.push(DEFAULT_PD_ENDPOINTS.to_string());
        }
        Self {
            pd_endpoints,
            security: TikvSecurity::from_env(),
            request_timeout: request_timeout_from_env(),
            grpc_max_decoding_message_size: env_parsed(
                "KASANE_TIKV_GRPC_MAX_DECODING_MESSAGE_SIZE",
                DEFAULT_GRPC_MAX_DECODING_MESSAGE_SIZE,
            ),
        }
    }
}

/// 未設定・解析不能なら既定値（LMDB 側の同名関数と対）。
pub(super) fn env_parsed<T: std::str::FromStr>(name: &str, default: T) -> T {
    env_str(name)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(default)
}

/// 環境変数を文字列として読む。**空文字は未設定として扱う。**
///
/// `.env` に `X=` と書いたときに既定値へ落ちず「空の設定」になるのを防ぐ。
fn env_str(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.trim().is_empty())
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

/// TLS 資材のファイル名。ディレクトリ 1 つで指定させるので、名前は固定する。
const TLS_FILES: [&str; 3] = ["ca.crt", "client.crt", "client.key"];

impl TikvSecurity {
    /// `KASANE_TIKV_TLS_DIR` に [`TLS_FILES`] が揃っていれば TLS を使う。
    ///
    /// パスを 3 つ別々に受け取っていたときは、一部だけ設定すると**黙って平文接続へ落ちた**。
    /// ディレクトリ 1 つなら、設定したのに TLS にならない状態は「ファイルが無い」だけになり、
    /// それは警告で名指しできる。
    fn from_env() -> Option<Self> {
        let dir = PathBuf::from(env_str("KASANE_TIKV_TLS_DIR")?);
        let [ca_path, cert_path, key_path] = TLS_FILES.map(|name| dir.join(name));

        if let Some(missing) = [&ca_path, &cert_path, &key_path]
            .into_iter()
            .find(|p| !p.is_file())
        {
            tracing::warn!(
                "KASANE_TIKV_TLS_DIR is set but {} is missing; connecting without TLS",
                missing.display()
            );
            return None;
        }

        Some(Self {
            ca_path,
            cert_path,
            key_path,
        })
    }
}

impl TikvDb {
    #[tracing::instrument(skip_all, fields(pd_endpoints = ?config.pd_endpoints))]
    pub async fn connect(config: TikvConfig) -> Result<Self, AppError> {
        let mut client_config = Config::default()
            .with_timeout(config.request_timeout)
            .with_grpc_max_decoding_message_size(config.grpc_max_decoding_message_size);
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

    #[tracing::instrument(skip_all)]
    async fn ensure_initialized(&self) -> Result<(), AppError> {
        // 版の確認は他のどの読み書きより先に行う。読み方が違うまま触ると壊れる。
        let (version, initialized) = self
            .read(async |r| Ok((r.schema_version().await?, r.cluster_initialized().await?)))
            .await?;

        // LMDB 側と同じ失敗を同じ型で返す（`lmdb::init` を参照）。
        match version {
            Some(SCHEMA_VERSION) => return Ok(()),
            // 版が違う、あるいは版を持たないのに初期化済み（版を刻む前の世代）。
            Some(_) => return Err(mismatch(version)),
            None if initialized => return Err(mismatch(version)),
            None => {}
        }

        let hash = hash_password(&root_password())?;
        let id = PrincipalId(uuid::Uuid::now_v7());

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

fn mismatch(found: Option<u32>) -> AppError {
    AppError::SchemaVersionMismatch {
        found,
        expected: SCHEMA_VERSION,
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

    pub(super) async fn schema_version(&self) -> Result<Option<u32>, AppError> {
        let Some(bytes) = kv::get(&self.txn, keys::schema_version()).await? else {
            return Ok(None);
        };
        let raw: [u8; 4] = bytes.as_slice().try_into().map_err(|_| {
            AppError::InternalError("schema version marker is not four bytes".to_string())
        })?;
        Ok(Some(u32::from_le_bytes(raw)))
    }
}

impl TikvWrite<'_> {
    #[tracing::instrument(skip_all, fields(username = %username))]
    async fn initialize_cluster(
        &mut self,
        username: &str,
        id: PrincipalId,
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

        let already_seeded = self.user_record(username).await?.is_some();
        if !already_seeded {
            self.create_user(username, id, password_hash, privileges)
                .await?;
        }

        kv::put_many(
            &self.txn,
            [
                (keys::cluster_initialized(), MARKER_PRESENT.to_vec()),
                (
                    keys::schema_version(),
                    SCHEMA_VERSION.to_le_bytes().to_vec(),
                ),
            ],
        )
        .await;
        Ok(!already_seeded)
    }
}
