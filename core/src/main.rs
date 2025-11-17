use std::{env, path::PathBuf};
use tokio::sync::watch;

use crate::operation::{configuration::configuration, kasane::kasane};

pub mod command;
pub mod interface;
pub mod io;
pub mod macros;
pub mod operation;
pub mod user_error;

#[tokio::main]
async fn main() {
    // 設定読み込み
    let conf = configuration();

    // ストレージファイルのパス設定
    let file = PathBuf::from("default.kasane");

    // シャットダウン用の watch チャンネル
    let (shutdown_tx, shutdown_rx) = watch::channel(());

    // Ctrl+C シグナルを受け取ったらシャットダウン
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        println!("Ctrl+C detected. Shutting down...");
        let _ = shutdown_tx.send(());
    });

    // サーバー起動
    kasane(shutdown_rx, conf, file).await;
}
