use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::{Parser, Subcommand};
use std::{
    fs,
    net::SocketAddr,
    path::Path,
    process::{exit, Command},
};
pub mod operation;
use tokio::net::TcpListener;
use toml_edit::{value, Document, DocumentMut};

use crate::operation::{
    kasane::{self, kasane},
    setting::configuration,
};

pub mod command;
pub mod io;
pub mod json;
pub mod macros;
pub mod user_error;

const PID_FILE: &str = "kasane.pid";

#[derive(Parser)]
#[command(name = "kasane")]
#[command(about = "WebSocket server control")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    ///kasaneを起動する
    Up,

    ///kasaneを終了する
    Down,

    ///kasane.tomlをチェックする
    Check,

    ///kasane.tomlを適応する
    Apply,

    ///kasane.tomlをリセットする
    Init,

    ///kasaneの全てのデータをエクスポートする
    Export,

    ///kasaneのバックアップデータをインポートする
    Import,

    ///kasaneの現在のステータスを表示する
    Status,

    ///kasane-viewを開く
    View,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let conf = configuration();

    match cli.command {
        Some(Commands::Up) => operation::cli::up::up(),
        Some(Commands::Down) => {
            #[cfg(windows)]
            {
                let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());
                operation::cli::down::down(&shutdown_tx);
            }
            #[cfg(unix)]
            operation::cli::down::down();
        }
        Some(Commands::Check) => todo!(),
        Some(Commands::Apply) => todo!(),
        Some(Commands::Init) => todo!(),
        Some(Commands::Export) => todo!(),
        Some(Commands::Import) => todo!(),
        Some(Commands::Status) => todo!(),
        Some(Commands::View) => todo!(),

        // サブコマンドが指定されなかった場合は kasane 関数を起動
        None => {
            #[cfg(windows)]
            {
                let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
                kasane::kasane(shutdown_rx, conf).await;
            }
            #[cfg(unix)]
            {
                kasane::kasane_unix().await;
            }
        }
    }
}
