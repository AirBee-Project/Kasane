use serde::Deserialize;
use std::fs;
use toml_edit::{Document, DocumentMut, Item, Table};

#[derive(Debug, Deserialize)]
pub struct Configuration {
    pub title: String,
    pub network: Network,
    pub cpu: CPU,
}

#[derive(Debug, Deserialize)]
pub struct Network {
    pub connection_pool: usize,
    pub port: usize,
}

#[derive(Debug, Deserialize)]
pub struct CPU {
    pub core: usize,
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

    // デフォルト値をセット（未設定の場合のみ）
    if doc.get("title").is_none() {
        doc["title"] = "Kasane Configuration".into();
    }

    if doc.get("network").is_none() {
        doc["network"] = Item::Table(Table::new());
    }

    if doc["network"].get("connection_pool").is_none() {
        doc["network"]["connection_pool"] = 10.into();
    }

    if doc["network"].get("port").is_none() {
        doc["network"]["port"] = 3000.into();
    }

    if doc.get("cpu").is_none() {
        doc["cpu"] = Item::Table(Table::new());
    }

    if doc["cpu"].get("core").is_none() {
        doc["cpu"]["core"] = 4.into();
    }

    // ファイルに書き戻す
    fs::write("kasane.toml", doc.to_string()).unwrap();

    // toml_edit::Document を文字列化して構造体に変換
    let setting: Configuration = toml::from_str(&doc.to_string())
        .expect("Failed to deserialize kasane.toml into Configuration");

    setting
}
