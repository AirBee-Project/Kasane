//! 書き込み経路のベンチマーク（group commit / 分割閾値の効果測定）。
//!
//! 実クラスタが要るので `#[ignore]` にしてある。ローカルの Docker で回す:
//!
//! ```bash
//! docker compose -f deployment/tikv/docker-compose.yml up -d
//! cargo test --release --no-default-features --features backend-tikv \
//!     --test bench_write -- --ignored --nocapture
//! ```
//!
//! # 何を測るか
//!
//! PLATEAU の一括投入と同じ形、つまり **空間的に近い ID へ多数の小さな書き込みが
//! 同時に飛ぶ** 状況を作る。この形が効くのは、ツリーが 1 件の変更でもリーフを丸ごと
//! 書き直すため、同じリーフを狙う N 個のトランザクションが
//!
//! - リーフのサイズ × N バイトを書き（書き込み増幅）
//! - 同じリーフのロックを奪い合う（競合とやり直し）
//!
//! という二重の損をするから。畳み込みはこの両方を同時に潰す。
//!
//! # つまみ
//!
//! - `KASANE_WRITE_BATCH`: 1 バッチの上限。`1` で畳み込み前と同じ挙動になる
//! - `KASANE_BENCH_WRITERS`: 同時に飛ばす要求数（既定 64）
//! - `KASANE_BENCH_TOTAL`: 総書き込み件数（既定 4096）

#![cfg(feature = "backend-tikv")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Endpoint};

use kasane::grpc::pb;
use kasane::repositories::tikv::{TikvConfig, TikvDb};
use kasane::repositories::{Storage, WriteRepository};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

#[derive(Clone)]
struct TokenInterceptor(String);

impl Interceptor for TokenInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if let Ok(value) = self.0.parse() {
            req.metadata_mut().insert("authorization", value);
        }
        Ok(req)
    }
}

type DataClient =
    pb::data_service_client::DataServiceClient<InterceptedService<Channel, TokenInterceptor>>;

/// gRPC サーバーを ephemeral ポートで起動し、root トークン付きの `DataServiceClient` を返す。
async fn spawn_server(db: TikvDb) -> (DataClient, tokio::sync::oneshot::Sender<()>) {
    let app_state = kasane::AppState::new(db);
    let token = kasane::services::auth::generate_jwt(&app_state, "root")
        .await
        .unwrap();

    let bound = kasane::grpc::bind(app_state, "127.0.0.1:0".parse().unwrap())
        .expect("failed to bind an ephemeral port for the bench gRPC server");
    let addr = bound.local_addr();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = bound
            .serve(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let channel = Endpoint::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to the bench gRPC server");

    let client = pb::data_service_client::DataServiceClient::with_interceptor(
        channel,
        TokenInterceptor(format!("Bearer {token}")),
    );

    (client, shutdown_tx)
}

/// PD から全ストアの使用量合計を取る。取れなければ `None`（計測は続行する）。
///
/// MVCC は上書きを古い版として残すので、短時間で見た使用量の増分は
/// **実際に書いたバイト数**にほぼ比例する。書き込み増幅がそのまま出る。
/// `endpoint` は接続に使うのと同じ `TikvConfig` から取る。ここで環境変数を
/// 読み直すと、既定値がこのファイルにもう 1 つできて必ずずれる。
async fn store_used_bytes(endpoint: &str) -> Option<u64> {
    // 依存を増やしたくないので curl に任せる。取れなければ黙って諦める。
    let out = std::process::Command::new("curl")
        .args(["-s", &format!("http://{endpoint}/pd/api/v1/stores")])
        .output()
        .ok()?;
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;

    let total = body
        .get("stores")?
        .as_array()?
        .iter()
        .filter_map(|s| parse_size(s.pointer("/status/used_size")?.as_str()?))
        .sum();
    Some(total)
}

/// PD が返す `"6.079GiB"` のような人が読む形の大きさをバイトへ直す。
fn parse_size(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    let split = raw.find(|c: char| c.is_alphabetic()).unwrap_or(raw.len());
    let value: f64 = raw[..split].parse().ok()?;
    let unit = match raw[split..].trim() {
        "" | "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((value * unit) as u64)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "ローカルの TiKV クラスタが要る"]
async fn bench_concurrent_writes() {
    let writers = env_usize("KASANE_BENCH_WRITERS", 64);
    let total = env_usize("KASANE_BENCH_TOTAL", 4096);
    let batch = std::env::var("KASANE_WRITE_BATCH").unwrap_or_else(|_| "256 (既定)".into());

    let config = TikvConfig::from_env();
    let endpoint = config.pd_endpoints.first().cloned();
    let db = TikvDb::connect(config).await.expect("TiKV に接続できない");

    let db_name = format!("bench_{}", uuid::Uuid::now_v7().simple());
    {
        let db_name = db_name.clone();
        db.write(async move |w| {
            w.database_create(&db_name, None).await?;
            w.table_create(
                &db_name,
                "t",
                kasane::models::database::table::TableDataType::Int,
                25,
                None,
                None,
                false,
                true,
            )
            .await
        })
        .await
        .unwrap();
    }

    let (client, _shutdown) = spawn_server(db.clone()).await;

    let measure = async |at: &Option<String>| match at {
        Some(endpoint) => store_used_bytes(endpoint).await,
        None => None,
    };
    let before = measure(&endpoint).await;

    let next = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(AtomicUsize::new(0));

    let started = Instant::now();
    let mut tasks = Vec::with_capacity(writers);
    for _ in 0..writers {
        let (mut client, db_name) = (client.clone(), db_name.clone());
        let (next, failures) = (next.clone(), failures.clone());

        // 遅延は各タスクが自前で持ち、合流してから束ねる。共有ロックを計測ループの
        // 中に置くと、測っている当のものを測定器が汚す。
        tasks.push(tokio::spawn(async move {
            let mut mine = Vec::<Duration>::new();
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= total {
                    break;
                }

                // 空間的に密な ID。連番の x なので同じリーフに集中し、
                // PLATEAU の一括投入と同じ「同じシャードの奪い合い」を再現する。
                let request = pb::InsertDataRequest {
                    db_name: db_name.clone(),
                    table_name: "t".to_string(),
                    value: Some(pb::TypedValue {
                        kind: Some(pb::typed_value::Kind::IntVal(i as i64)),
                    }),
                    spatial_ids: vec![pb::SpatialId {
                        kind: Some(pb::spatial_id::Kind::SingleId(pb::SingleId {
                            z: 20,
                            f: 0,
                            x: i as u32,
                            y: 0,
                            i: None,
                            t: None,
                        })),
                    }],
                    zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
                };

                let at = Instant::now();
                let result = client.insert(request).await;
                let elapsed = at.elapsed();

                if let Err(status) = result {
                    // 最初の 1 件だけ中身を出す。全滅しているのが競合ではなく
                    // 要求の作り方の間違い、という取り違えを防ぐため。
                    if failures.fetch_add(1, Ordering::Relaxed) == 0 {
                        eprintln!("最初の失敗: {} {}", status.code(), status.message());
                    }
                }
                mine.push(elapsed);
            }
            mine
        }));
    }
    let mut latencies = Vec::<Duration>::with_capacity(total);
    for task in tasks {
        latencies.extend(task.await.unwrap());
    }
    let wall = started.elapsed();

    // ストアの使用量は heartbeat（既定 10 秒）でしか PD に届かない。
    // 直後に読むと書いたぶんが載っていないので、1 周期ぶん待つ。
    if before.is_some() {
        tokio::time::sleep(Duration::from_secs(20)).await;
    }
    let after = measure(&endpoint).await;

    latencies.sort_unstable();
    let at = |q: f64| latencies[((latencies.len() - 1) as f64 * q) as usize];

    println!("\n===== 書き込みベンチ =====");
    println!("KASANE_WRITE_BATCH : {batch}");
    println!(
        "MAX_FLEX_ID_PER_SHARD: {}",
        kasane::repositories::encoding::shard_entry::MAX_FLEX_ID_PER_SHARD
    );
    println!("同時要求数         : {writers}");
    println!("総書き込み件数     : {total}");
    println!("----------------------------------------");
    println!("経過時間           : {:.2} s", wall.as_secs_f64());
    println!(
        "スループット       : {:.0} writes/s",
        total as f64 / wall.as_secs_f64()
    );
    println!("失敗               : {}", failures.load(Ordering::Relaxed));
    println!("遅延 p50           : {:?}", at(0.50));
    println!("遅延 p95           : {:?}", at(0.95));
    println!("遅延 p99           : {:?}", at(0.99));
    println!("遅延 max           : {:?}", latencies[latencies.len() - 1]);
    if let (Some(before), Some(after)) = (before, after) {
        let grew = after.saturating_sub(before);
        println!("----------------------------------------");
        println!(
            "ストア使用量の増分 : {:.1} MiB ({:.0} bytes/write)",
            grew as f64 / (1024.0 * 1024.0),
            grew as f64 / total as f64
        );
    }
    println!("========================================\n");

    let db_name2 = db_name.clone();
    let _ = db
        .write(async move |w| w.database_remove(&db_name2).await)
        .await;

    assert_eq!(
        failures.load(Ordering::Relaxed),
        0,
        "書き込みが失敗している"
    );
}
