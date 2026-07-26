//! `.bruno/Query` に置いたリクエスト例が、実際の [`ExecuteQueryRequest`] として
//! 解釈できることを検証する。
//!
//! Bruno コレクションはコンパイル対象ではないため、モデルを変更しても気付かないまま
//! 古い形のままになりやすい（実際 `convert` や `zoom_level_policy` の削除で陳腐化しうる）。
//! ここで実物の型に通しておけば、モデル変更時にテストが落ちて気付ける。

use kasane::models::query::ExecuteQueryRequest;

/// 1リクエストファイルから、本体と各 example のリクエストボディを取り出す。
fn json_bodies(doc: &serde_yaml::Value) -> Vec<(String, String)> {
    let mut out = Vec::new();

    let body_of = |v: &serde_yaml::Value| -> Option<String> {
        let body = v.get("body")?;
        if body.get("type")?.as_str()? != "json" {
            return None;
        }
        Some(body.get("data")?.as_str()?.to_string())
    };

    if let Some(http) = doc.get("http")
        && let Some(data) = body_of(http)
    {
        out.push(("http".to_string(), data));
    }

    if let Some(examples) = doc.get("examples").and_then(|v| v.as_sequence()) {
        for ex in examples {
            let name = ex
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("<unnamed>");
            if let Some(req) = ex.get("request")
                && let Some(data) = body_of(req)
            {
                out.push((format!("example[{name}]"), data));
            }
        }
    }

    out
}

#[test]
fn bruno_query_requests_match_the_api_model() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".bruno/Query");
    let mut checked = 0usize;

    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("`.bruno/Query` が見つからない")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml"))
        .filter(|p| p.file_name().is_some_and(|n| n != "folder.yml"))
        .collect();
    entries.sort();

    assert!(!entries.is_empty(), "`.bruno/Query` にリクエストが無い");

    for path in entries {
        let text = std::fs::read_to_string(&path).unwrap();
        let doc: serde_yaml::Value = serde_yaml::from_str(&text)
            .unwrap_or_else(|e| panic!("{}: YAML 不正: {e}", path.display()));

        let file = path.file_name().unwrap().to_string_lossy().to_string();

        // すべて POST /query を叩いていること
        let url = doc["http"]["url"].as_str().unwrap_or_default();
        assert!(
            url.ends_with("/query"),
            "{file}: url が /query でない: {url}"
        );
        assert_eq!(doc["http"]["method"].as_str(), Some("POST"), "{file}");

        let bodies = json_bodies(&doc);
        assert!(!bodies.is_empty(), "{file}: JSON ボディが無い");

        for (where_, data) in bodies {
            serde_json::from_str::<ExecuteQueryRequest>(&data).unwrap_or_else(|e| {
                panic!("{file} / {where_}: ExecuteQueryRequest として解釈できない: {e}\n{data}")
            });
            checked += 1;
        }
    }

    assert!(checked >= 6, "検証したボディが少なすぎる: {checked}");
}
