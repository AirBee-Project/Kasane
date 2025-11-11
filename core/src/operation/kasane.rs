use axum::{
    extract::State,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::{
    env::{self, current_dir},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{
    command::process,
    io::full::Storage,
    json::{input::Packet, output::Output},
    operation::{
        configuration::Configuration,
        login::{login, Claims},
    },
    user_error::UserError,
};

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
    cmd: crate::json::input::Command,
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
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            jwt_auth_middleware,
        ))
        .route("/login", post(login))
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

use axum::body::Body;

async fn jwt_auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
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

    let token = auth_header.strip_prefix("Bearer ").ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid authorization format".to_string(),
    ))?;

    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(
            dotenvy::var("JWT_SECRET")
                .expect("JWT_SECRET must be set")
                .as_bytes(),
        ),
        &Validation::default(),
    )
    .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)))?
    .claims;

    state
        .storage
        .validate_session(&claims.session_id)
        .map_err(|e| {
            (
                StatusCode::UNAUTHORIZED,
                format!("Session expired or invalid: {}", e),
            )
        })?;

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
            storage: state.storage.clone(),
            resp: resp_tx,
        };

        //Todoここで権限を検証

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
