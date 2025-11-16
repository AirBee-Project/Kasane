use serde::Deserialize;
use std::fs;
use toml_edit::{DocumentMut, Item, Table};

#[derive(Debug, Deserialize, Clone)]
pub struct Configuration {
    pub network: Network,
    pub general: General,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Network {
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct General {
    /// 指定されなかった場合は最大のCPU数を使用する
    pub cpu_num: Option<usize>,
    pub queue_size: usize,
    pub session_expiration_secs: u64,
}

///kasane.tomlが存在する場合はその設定を読み込む
///存在しないパラメーターについてはデフォルト値に設定する
pub fn configuration() -> Configuration {
    // kasane.toml を読み込む。存在しなければ空文字列
    let original = fs::read_to_string("kasane.toml").unwrap_or_else(|_| {
        println!("kasane.toml was not found. A new default configuration has been created.");
        "".to_string()
    });

    // toml_edit でパース
    let mut doc = original.parse::<DocumentMut>().unwrap_or_else(|_| {
        panic!("Failed to load kasane.toml correctly. Run `kasane check` to identify issues, or use `kasane init` to reset the configuration.");
    });

    if doc.get("network").is_none() {
        doc["network"] = Item::Table(Table::new());
    }

    if doc["network"].get("max_keepalive_sessions").is_none() {
        doc["network"]["max_keepalive_sessions"] = 30.into();
    }

    if doc["network"].get("port").is_none() {
        doc["network"]["port"] = 3000.into();
    }

    if doc.get("general").is_none() {
        doc["general"] = Item::Table(Table::new());
    }

    if doc["general"].get("queue_size").is_none() {
        doc["general"]["queue_size"] = 1024.into();
    }

    if doc["general"].get("session_expiration_secs").is_none() {
        doc["general"]["session_expiration_secs"] = 3600.into();
    }

    // ファイルに書き戻す
    fs::write("kasane.toml", doc.to_string()).unwrap();

    // toml_edit::Document を文字列化して構造体に変換
    let setting: Configuration = toml::from_str(&doc.to_string())
        .expect("Failed to deserialize kasane.toml into Configuration");

    setting
}
