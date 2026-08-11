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

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use kasane::repositories::tikv::{TikvConfig, TikvDb};
use kasane::repositories::{Storage, WriteRepository};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

/// 全リクエストに root の資格情報を差し込んだルータを作る。
async fn build_app(db: TikvDb) -> Router {
    let app_state = kasane::AppState::new(db);
    let token = kasane::services::auth::generate_jwt(&app_state, "root")
        .await
        .unwrap();
    let auth = axum::http::HeaderValue::from_str(&format!("Bearer {token}")).unwrap();

    kasane::kasane(app_state).layer(axum::middleware::from_fn(
        move |mut req: axum::extract::Request, next: axum::middleware::Next| {
            let auth = auth.clone();
            async move {
                req.headers_mut()
                    .insert(axum::http::header::AUTHORIZATION, auth);
                next.run(req).await
            }
        },
    ))
}

/// PD から全ストアの使用量合計を取る。取れなければ `None`（計測は続行する）。
///
/// MVCC は上書きを古い版として残すので、短時間で見た使用量の増分は
/// **実際に書いたバイト数**にほぼ比例する。書き込み増幅がそのまま出る。
async fn store_used_bytes() -> Option<u64> {
    let endpoint = std::env::var("KASANE_TIKV_PD_ENDPOINTS")
        .unwrap_or_else(|_| "127.0.0.1:2379".to_string())
        .split(',')
        .next()?
        .trim()
        .to_string();

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

    let db = TikvDb::connect(TikvConfig::from_env())
        .await
        .expect("TiKV に接続できない");

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
            )
            .await
        })
        .await
        .unwrap();
    }

    let app = build_app(db.clone()).await;
    let uri = format!("/databases/{db_name}/tables/t/data");

    let before = store_used_bytes().await;

    let next = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(AtomicUsize::new(0));
    let latencies = Arc::new(std::sync::Mutex::new(Vec::<Duration>::with_capacity(total)));

    let started = Instant::now();
    let mut tasks = Vec::with_capacity(writers);
    for _ in 0..writers {
        let (app, uri) = (app.clone(), uri.clone());
        let (next, failures, latencies) = (next.clone(), failures.clone(), latencies.clone());

        tasks.push(tokio::spawn(async move {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= total {
                    break;
                }

                // 空間的に密な ID。連番の x なので同じリーフに集中し、
                // PLATEAU の一括投入と同じ「同じシャードの奪い合い」を再現する。
                let body = serde_json::json!({
                    "value": i as i64,
                    "spatial_ids": [{
                        "type": "singleId", "z": 20, "f": 0, "x": i, "y": 0
                    }],
                });

                let req = Request::builder()
                    .method("PUT")
                    .uri(&uri)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap();

                let at = Instant::now();
                let response = app.clone().oneshot(req).await.unwrap();
                let elapsed = at.elapsed();

                if response.status() != StatusCode::OK && response.status() != StatusCode::CREATED {
                    // 最初の 1 件だけ中身を出す。全滅しているのが競合ではなく
                    // 要求の作り方の間違い、という取り違えを防ぐため。
                    if failures.fetch_add(1, Ordering::Relaxed) == 0 {
                        let status = response.status();
                        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
                            .await
                            .unwrap_or_default();
                        eprintln!("最初の失敗: {status} {}", String::from_utf8_lossy(&body));
                    }
                }
                latencies.lock().unwrap().push(elapsed);
            }
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    let wall = started.elapsed();

    // ストアの使用量は heartbeat（既定 10 秒）でしか PD に届かない。
    // 直後に読むと書いたぶんが載っていないので、1 周期ぶん待つ。
    if before.is_some() {
        tokio::time::sleep(Duration::from_secs(20)).await;
    }
    let after = store_used_bytes().await;

    let mut latencies = latencies.lock().unwrap().clone();
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
