use axum::{
    extract::State,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot, watch};
use uuid::Uuid;

use crate::{
    command::process,
    io::full::Storage,
    json::{input::Packet, output::Output},
    operation::setting::Configuration,
    user_error::UserError,
};

// ==========================
// 設定
// ==========================
const MAX_KEEPALIVE_SESSIONS: usize = 30; // Keep-alive維持する最大セッション数
const JWT_EXPIRATION_HOURS: u64 = 1; // JWT有効期限（Keep-alive用）
const JWT_EXPIRATION_MINUTES: u64 = 5; // JWT有効期限（通常用）
const JWT_SECRET: &[u8] = b"your-secret-key-change-this-in-production"; // 本番環境では環境変数から読み込むこと
const WORKER_QUEUE_SIZE: usize = 1000;

#[derive(Clone)]
struct AppState {
    storage: Arc<Storage>,
    job_sender: JobSender,
}

#[derive(Clone)]
struct JobSender {
    tx: mpsc::Sender<Job>,
}

struct Job {
    cmd: crate::json::input::Command,
    storage: Arc<Storage>,
    session_id: String,
    resp: oneshot::Sender<Result<Output, UserError>>,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    token_type: String,
    expires_in: u64,
}

// JWT クレーム
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: String,        // ユーザー名
    session_id: String, // セッションID（ストレージで管理）
    exp: u64,           // 有効期限 (UNIX timestamp)
    iat: u64,           // 発行時刻 (UNIX timestamp)
    is_keepalive: bool, // Keep-alive対象かどうか
}

impl IntoResponse for UserError {
    fn into_response(self) -> Response {
        let message = self.to_string();
        (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![message])).into_response()
    }
}

pub async fn kasane(mut shutdown: watch::Receiver<()>, conf: Configuration) {
    println!("RESTful API server started");

    let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();

    // ストレージを初期化
    let storage = Arc::new(match Storage::new(None) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ストレージの初期化に失敗しました: {:?}", e);
            return;
        }
    });

    // ワーカープールの構築
    let (tx, rx) = mpsc::channel::<Job>(WORKER_QUEUE_SIZE);
    let job_sender = JobSender { tx };

    let rx = Arc::new(tokio::sync::Mutex::new(rx));
    let cpu_cores = num_cpus::get();

    println!("Starting {} worker threads", cpu_cores);

    // CPU コア数分のワーカースレッドを起動
    for worker_id in 0..cpu_cores {
        let rx = Arc::clone(&rx);
        tokio::spawn(async move {
            println!("Worker {} started", worker_id);
            loop {
                let job_opt = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };

                match job_opt {
                    Some(job) => {
                        let storage = job.storage.clone();
                        let cmd = job.cmd.clone();

                        // CPU負荷の高い処理はブロッキングスレッドで実行
                        let result = tokio::task::spawn_blocking(move || {
                            process(cmd, storage, job.session_id)
                        })
                        .await;

                        let response = match result {
                            Ok(output) => output.await,
                            Err(_) => Err(UserError::QueueReceiveError {
                                location: format!("worker_{}", worker_id),
                            }),
                        };

                        // 結果を送信（エラーは無視）
                        let _ = job.resp.send(response);
                    }
                    None => {
                        println!("Worker {} shutting down", worker_id);
                        break;
                    }
                }
            }
        });
    }

    // アプリケーション状態
    let app_state = AppState {
        storage: storage.clone(),
        job_sender,
    };

    // ルーター構築
    let app = Router::new()
        .route("/", post(execute_handler))
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            jwt_auth_middleware,
        ))
        .route("/login", post(login_handler))
        .with_state(app_state);

    // Graceful shutdown
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.changed().await.ok();
            println!("Shutdown signal received");
        })
        .await
        .unwrap();

    println!("RESTful API server gracefully stopped");
}

async fn jwt_auth_middleware<B>(
    State(state): State<AppState>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, (StatusCode, String)> {
    // Authorization ヘッダーからトークンを取得
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing authorization header".to_string(),
        ))?;

    // "Bearer <token>" 形式をパース
    let token = auth_header.strip_prefix("Bearer ").ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid authorization format".to_string(),
    ))?;

    // JWT を検証
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET),
        &Validation::default(),
    )
    .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)))?
    .claims;

    // ストレージでセッションを検証（二重チェック）
    state
        .storage
        .validate_session(&claims.session_id)
        .map_err(|e| {
            (
                StatusCode::UNAUTHORIZED,
                format!("Session expired or invalid: {}", e),
            )
        })?;

    // リクエストの拡張データにクレームを追加
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// POST / - コマンド実行エンドポイント
async fn execute_handler(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Json(packet): Json<Packet>,
) -> Result<Json<Vec<Result<Output, UserError>>>, (StatusCode, String)> {
    // JWT認証 + ストレージセッション検証済み
    // claims.sub にユーザー名、claims.session_id にセッションID

    println!(
        "Authenticated user: {} (session: {})",
        claims.sub, claims.session_id
    );

    // コマンドを順次処理
    let mut results = Vec::with_capacity(packet.command.len());

    for cmd in packet.command {
        let (resp_tx, resp_rx) = oneshot::channel();
        let job = Job {
            cmd,
            session_id: claims.session_id,
            storage: state.storage.clone(),
            resp: resp_tx,
        };

        // ジョブをキューに送信
        if let Err(_) = state.job_sender.tx.send(job).await {
            results.push(Err(UserError::QueueSendError {
                location: "execute_handler".to_string(),
            }));
            continue;
        }

        // 結果を受信（順次処理）
        match resp_rx.await {
            Ok(res) => results.push(res),
            Err(_) => results.push(Err(UserError::QueueReceiveError {
                location: "execute_handler".to_string(),
            })),
        }
    }

    Ok(Json(results))
}
