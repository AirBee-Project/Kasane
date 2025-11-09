use std::{fs, path::Path, process::Command};

use crate::{operation::setting::Configuration, PID_FILE};

pub fn up() {
    // すでに起動中か確認
    if Path::new(PID_FILE).exists() {
        println!("Kasane is already running.");
        return;
    }

    // 自分自身のバイナリパスを取得
    let exe_path = std::env::current_exe().expect("Failed to get current exe path");

    // 子プロセスとして起動
    let child = Command::new(exe_path)
        .arg("run") // 内部的に WebSocket サーバーを起動するサブコマンド
        .spawn()
        .expect("Failed to start WebSocket server");

    // PID をファイルに保存
    fs::write(PID_FILE, child.id().to_string()).expect("Failed to write PID file");

    println!("Kasane WebSocket server started with PID {}", child.id());
}
