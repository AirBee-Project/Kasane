use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::SystemTime};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot, watch};
use uuid::Uuid;

use crate::{
    command::process,
    interface::{
        input::{Command, DatabaseCommand, Packet},
        output::Output,
    },
    io::full::Storage,
    operation::configuration::Configuration,
    user_error::UserError,
};

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

// レスポンス
#[derive(Serialize)]
struct LoginResponse {
    session_id: String,
    expires_at: u64,
}

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<Storage>,
    job_sender: JobSender,
    pub conf: Configuration,
}

#[derive(Clone)]
struct JobSender {
    tx: mpsc::Sender<Job>,
}

struct Job {
    cmd: crate::interface::input::Command,
    storage: Arc<Storage>,
    resp: oneshot::Sender<Result<Output, UserError>>,
}

impl IntoResponse for UserError {
    fn into_response(self) -> Response {
        let message = self.to_string();
        (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![message])).into_response()
    }
}

pub async fn kasane(mut shutdown: watch::Receiver<()>, conf: Configuration, file: PathBuf) {
    println!("RESTful API server started");

    let addr: SocketAddr =
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), conf.network.port);

    // ストレージを初期化

    let storage = Arc::new(match Storage::new(file) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ストレージの初期化に失敗しました: {:?}", e);
            return;
        }
    });

    //開発環境用にアカウントを作成
    let _ = storage.create_user("admin", "admin");

    // ワーカープールの構築
    let (tx, rx) = mpsc::channel::<Job>(conf.general.queue_size);
    let job_sender = JobSender { tx };

    let rx = Arc::new(tokio::sync::Mutex::new(rx));

    //使用するCPUのコア数を設定する
    let cpu_num = match conf.general.cpu_num {
        Some(v) => {
            if v > num_cpus::get() {
                num_cpus::get()
            } else {
                v
            }
        }
        None => num_cpus::get(),
    };

    // CPU コア数分のワーカースレッドを起動
    for worker_id in 0..cpu_num {
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
                        let result =
                            tokio::task::spawn_blocking(move || process(cmd, storage)).await;

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
        conf,
    };

    // ルーター構築
    let app = Router::new()
        .route("/", post(execute_handler))
        .route("/login", post(login))
        .with_state(app_state);

    // Graceful shutdown
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");
    println!("Listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.changed().await.ok();
            println!("Shutdown signal received");
        })
        .await
        .expect("Failed to serve application");

    println!("RESTful API server gracefully stopped");
}

// ログインハンドラー
async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let session_id = Uuid::new_v4();

    let expires_at = match state.storage.create_session(
        &payload.username,
        &payload.password,
        &session_id,
        state.conf.general.session_expiration_secs,
    ) {
        Ok(v) => v,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Invalid or expired session".into(),
            ));
        }
    };

    let response = LoginResponse {
        session_id: session_id.to_string(),
        expires_at,
    };

    Ok(Json(response))
}

#[derive(Deserialize)]
struct ExecuteRequest {
    session_id: String,
    command: Vec<Command>,
}

/// POST / - コマンド実行エンドポイント
async fn execute_handler(
    State(state): State<AppState>,
    Json(packet): Json<ExecuteRequest>,
) -> Result<Json<Vec<Result<Output, UserError>>>, (StatusCode, String)> {
    // 1. セッション検証
    let user_id = match state.storage.verify_session(&packet.session_id) {
        Ok(uid) => uid,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Invalid or expired session".into(),
            ));
        }
    };

    // 2. コマンドを順次処理
    let mut results = Vec::with_capacity(packet.command.len());

    for cmd in packet.command {
        let (resp_tx, resp_rx) = oneshot::channel();
        let job = Job {
            cmd,
            storage: state.storage.clone(),
            resp: resp_tx,
        };

        if let Err(_) = state.job_sender.tx.send(job).await {
            results.push(Err(UserError::QueueSendError {
                location: "execute_handler".to_string(),
            }));
            continue;
        }

        match resp_rx.await {
            Ok(res) => results.push(res),
            Err(_) => results.push(Err(UserError::QueueReceiveError {
                location: "execute_handler".to_string(),
            })),
        }
    }

    Ok(Json(results))
}
