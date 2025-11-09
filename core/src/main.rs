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

use crate::operation::setting::configuration;

const PID_FILE: &str = "kasane.pid";

#[derive(Parser)]
#[command(name = "kasane")]
#[command(about = "WebSocket server control")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let conf = configuration();

    match cli.command {
        Commands::Up => {
            operation::up::up();
        }
        Commands::Down => {
            println!("end");
        }
        Commands::Check => todo!(),
        Commands::Apply => todo!(),
        Commands::Init => todo!(),
        Commands::Export => todo!(),
        Commands::Import => todo!(),
        Commands::Status => todo!(),
    }

    //本体プロセスを軌道
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "run" {
        // WebSocketサーバー起動
        run_websocket_server().await;
        return;
    }
}
