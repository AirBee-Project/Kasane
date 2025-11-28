use serde_json::Value;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn process_json(input: &str) -> String {
    // JSON文字列をパース
    let mut json: Value = serde_json::from_str(input).unwrap();

    // JSONにフィールドを追加
    json["status"] = Value::from("ok");

    // 返す
    serde_json::to_string(&json).unwrap()
}
