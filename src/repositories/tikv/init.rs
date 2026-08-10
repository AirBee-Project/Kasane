//! TiKV 固有の初期化設定。
//!
//! 接続先の解決・クライアントの構築・既定ユーザーの投入といった
//! 「起動時に一度だけ行うこと」をここに集める。定常運用の読み書きは `mod.rs` 以下が担う。
//!
//! 接続設定の読み取りはこのモジュールに一本化してある（`backend.rs` は
//! [`TikvConfig::from_env`] を呼ぶだけ）。既定値や環境変数名が 2 箇所に分かれると、
//! 片方だけ直したときに黙って食い違うため。

use std::path::PathBuf;
use std::sync::Arc;

use tikv_client::{Config, TransactionClient};

use crate::error::AppError;
use crate::models::users::{PrivilegeRule, UserRole};
use crate::repositories::{CatalogRepository, Storage, WriteRepository};
use crate::services::auth::hash_password;

use super::keys::{self, LockScope};
use super::kv::{self, Reader};
use super::{TikvDb, TikvRead, TikvWrite, to_app_error};

/// PD エンドポイントの既定値。`KASANE_TIKV_PD_ENDPOINTS` で上書きできる。
pub(super) const DEFAULT_PD_ENDPOINTS: &str = "127.0.0.1:2379";

/// 初期投入する管理者ユーザーの名前。
const ROOT_USERNAME: &str = "root";

/// TiKV への接続設定。
#[derive(Debug, Clone)]
pub struct TikvConfig {
    /// PD のエンドポイント（`host:port`）。
    pub pd_endpoints: Vec<String>,
    /// TLS 用のファイル群（CA 証明書・クライアント証明書・秘密鍵）。
    ///
    /// `None` なら**平文接続**になる。TiKV のキースペースには認証の層が無いので、
    /// PD/TiKV へ到達できるホストはそのまま全データを読み書きできる。平文で運用してよいのは
    /// 経路自体が閉じている場合だけで、そうでなければここを設定すること。
    pub security: Option<TikvSecurity>,
}

/// TLS 相互認証に使うファイルの組。3 つ揃って初めて有効になる。
#[derive(Debug, Clone)]
pub struct TikvSecurity {
    pub ca_path: PathBuf,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl TikvConfig {
    /// 環境変数から組み立てる。
    ///
    /// | 変数 | 意味 |
    /// |---|---|
    /// | `KASANE_TIKV_PD_ENDPOINTS` | PD エンドポイント（カンマ区切り） |
    /// | `KASANE_TIKV_CA_PATH` | CA 証明書 |
    /// | `KASANE_TIKV_CERT_PATH` | クライアント証明書 |
    /// | `KASANE_TIKV_KEY_PATH` | クライアント秘密鍵 |
    pub fn from_env() -> Self {
        Self::from_endpoints(
            &std::env::var("KASANE_TIKV_PD_ENDPOINTS")
                .unwrap_or_else(|_| DEFAULT_PD_ENDPOINTS.to_string()),
        )
    }

    /// カンマ区切りのエンドポイント文字列から組み立てる。
    /// TLS 設定は環境変数から読む（接続先の指定方法とは独立に効かせたいため）。
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
    /// 3 つの環境変数が**すべて**設定されているときだけ有効になる。
    ///
    /// 一部だけの指定は設定ミスと見なしてよいが、ここで黙って平文へ落とすと
    /// 「TLS を設定したつもり」が起動ログにも残らない。`connect` 側で警告する。
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
                tracing::warn!(
                    "TiKV の TLS 設定が中途半端です。KASANE_TIKV_CA_PATH / \
                     KASANE_TIKV_CERT_PATH / KASANE_TIKV_KEY_PATH は 3 つとも指定してください。\
                     今回は平文で接続します"
                );
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

    /// クラスタをこのアプリ用に初期化する（未初期化なら root ユーザーを作る）。
    ///
    /// 判定に使うのは**初期化済みマーカー**であって `root` の有無ではない。
    /// 「`root` が居なければ作る」にすると、管理者が意図して `root` を消しても
    /// 次の起動で既定パスワードの管理者が復活してしまい、消せない管理者アカウントに
    /// なる。マーカーで判定すれば、初期投入は文字どおり 1 度きりになる。
    ///
    /// クラスタは複数の Kasane インスタンスで共有されうるので、同時起動しても
    /// 二重作成にならないよう通常の書き込み経路を通す（マーカーのロックで直列化され、
    /// 後から来た側は既に初期化済みであることを見て何もしない）。
    async fn ensure_initialized(&self) -> Result<(), AppError> {
        // 既に初期化済みなら、パスワードのハッシュ化コストを払わずに抜ける。
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
    /// 未初期化なら管理者ユーザーを作ってマーカーを立てる。作ったら `true`。
    ///
    /// **マーカーの確認より先にロックを取る**のが要点。ロックを取ってから始まる
    /// 作業トランザクションは前任者のコミットを必ず見るので（`mod.rs` の
    /// トランザクションの節）、同時起動しても初期投入は 1 度だけになる。
    async fn initialize_cluster(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<bool, AppError> {
        // 初期投入はこの管理者ユーザーを作ることなので、排他の単位もそのユーザーでよい。
        self.require_lock(LockScope::User, username.as_bytes())?;

        if kv::get(&self.txn, keys::cluster_initialized())
            .await?
            .is_some()
        {
            return Ok(false);
        }

        // マーカーは無いが管理者が既に居る場合。マーカーを導入する前のバージョンが
        // 初期化したクラスタなので、投入済みとして印だけ立てる。ここで作りにいくと
        // 既存クラスタが「ユーザーが既にいる」で起動できなくなる。
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
