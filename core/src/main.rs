use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
pub mod operation;

use crate::operation::{
    configuration::configuration,
    kasane::{self},
};
use dotenvy::from_filename;

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
    Up {
        #[arg(short, long, default_value_t = String::from("default.kasane"))]
        file: String,
    },

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
    //環境変数の読み込み
    load_env();

    let cli = Cli::parse();
    let conf = configuration();

    match cli.command {
        Some(Commands::Up { file }) => operation::cli::up::up(),
        Some(Commands::Down) => {
            #[cfg(windows)]
            {
                let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
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
                operation::cli::up::up();
            }
            #[cfg(unix)]
            {
                kasane::kasane_unix().await;
            }
        }
    }
}

fn load_env() {
    // リリースビルドかどうかでファイルを決定
    let env_file = if cfg!(debug_assertions) {
        ".env.example" // 開発用
    } else {
        ".env" // 本番用
    };

    if Path::new(env_file).exists() {
        from_filename(env_file).ok();
        println!("Loaded environment from {}", env_file);
    } else {
        println!("Environment file {} not found, skipping", env_file);
    }
}
