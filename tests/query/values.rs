//! 値に注目したクエリ（値フィルタ）と、全データ型への対応の検証。

use axum::http::StatusCode;

use super::{post_query, single_id, total_ids, values};
use crate::database::table::common::TestApp;
use crate::database::table::data::common::put_data;

/// `test_db` に指定型のテーブルを作り、`(x, 値)` を書き込む。
async fn seed(
    test_app: &TestApp,
    table: &str,
    data_type: &str,
    cells: &[(i64, serde_json::Value)],
) {
    test_app.create_table("test_db", table, data_type).await;
    for (x, v) in cells {
        put_data(
            test_app,
            table,
            &serde_json::json!({ "value": v, "spatial_ids": [single_id(*x)] }),
        )
        .await;
    }
}

fn source(table: &str) -> serde_json::Value {
    serde_json::json!({ "type": "source", "database": "test_db", "table": table })
}

fn ids(base: i64, count: i64) -> Vec<serde_json::Value> {
    (0..count).map(|i| single_id(base + i)).collect()
}

/// ある値のみを残す。
#[tokio::test]
async fn filter_equals_keeps_only_that_value() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_eq",
        "Int",
        &[
            (700000, serde_json::json!(1)),
            (700001, serde_json::json!(5)),
            (700002, serde_json::json!(10)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(700000, 3),
            "query": {
                "type": "filterValues", "mode": "equals", "value": 5,
                "input": source("t_eq")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![5]);
    assert_eq!(total_ids(&result), 1);
}

/// ある範囲の値を残す（閉区間）。
#[tokio::test]
async fn filter_in_range_is_inclusive() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_in",
        "Int",
        &[
            (710000, serde_json::json!(1)),
            (710001, serde_json::json!(5)),
            (710002, serde_json::json!(10)),
            (710003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(710000, 4),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 5, "max": 10,
                "input": source("t_in")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![5, 10], "境界を含むこと");
}

/// ある範囲の値**以外**を残す。
#[tokio::test]
async fn filter_not_in_range_keeps_the_outside() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_out",
        "Int",
        &[
            (720000, serde_json::json!(1)),
            (720001, serde_json::json!(5)),
            (720002, serde_json::json!(10)),
            (720003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(720000, 4),
            "query": {
                "type": "filterValues", "mode": "notInRange", "min": 5, "max": 10,
                "input": source("t_out")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![1, 20]);
}

/// Text テーブルでも値フィルタは使える（比較に必要なのは順序だけ）。
#[tokio::test]
async fn filter_works_on_text_values() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_words",
        "Text",
        &[
            (760000, serde_json::json!("apple")),
            (760001, serde_json::json!("banana")),
            (760002, serde_json::json!("cherry")),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(760000, 3),
            "query": {
                "type": "filterValues", "mode": "equals", "value": "banana",
                "input": source("t_words")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(result["dictionary"][0], serde_json::json!("banana"));
}

/// Boolean テーブルにもクエリを適用できる。
#[tokio::test]
async fn supports_boolean_tables() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_flag",
        "Boolean",
        &[
            (770000, serde_json::json!(true)),
            (770001, serde_json::json!(false)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(770000, 2),
            "query": {
                "type": "filterValues", "mode": "equals", "value": true,
                "input": source("t_flag")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(result["dictionary"][0], serde_json::json!(true));
}

/// Float テーブル（`Ord` ラッパー経由）にもクエリを適用できる。
#[tokio::test]
async fn supports_float_tables() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_float",
        "Float",
        &[
            (775000, serde_json::json!(1.5)),
            (775001, serde_json::json!(9.5)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(775000, 2),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 5.0,
                "input": source("t_float")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(result["dictionary"][0], serde_json::json!(9.5));
}

/// 64bit 相当の大きな整数値にもクエリを適用できる（`Int` = i64）。
#[tokio::test]
async fn supports_large_int_values() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_big",
        "Int",
        &[
            (778000, serde_json::json!(1_000_000_000_000i64)),
            (778001, serde_json::json!(1i64)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(778000, 2),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 1_000_000i64,
                "input": source("t_big")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 1);
    assert_eq!(
        result["dictionary"][0],
        serde_json::json!(1_000_000_000_000i64)
    );
}

/// 算術を要する演算子は非数値型では 400 になる。
#[tokio::test]
async fn rejects_arithmetic_operator_on_text() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_txt3", "Text", &[]).await;

    let (status, _) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(780000, 1),
            "query": {
                "type": "falloffLinearX", "z": 20, "radius": 2, "policy": "max",
                "input": source("t_txt3")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 数値専用の `MergePolicy` を非数値型に使うと 400 になる。
#[tokio::test]
async fn rejects_numeric_policy_on_text() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_txt4", "Text", &[]).await;

    let (status, _) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(785000, 1),
            "query": {
                "type": "zoomOut", "z": 19, "policy": "sum",
                "input": source("t_txt4")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// 演算子を挟まないクエリで、対象領域の全セルが返ること。
///
/// 対象領域の求め方を誤ると一部のセルだけが返る退行が起きるため、複数セルで固定する。
#[tokio::test]
async fn source_only_returns_all_cells() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_multi",
        "Int",
        &[
            (790000, serde_json::json!(1)),
            (790001, serde_json::json!(5)),
            (790002, serde_json::json!(10)),
            (790003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(790000, 4),
            "query": source("t_multi")
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![1, 5, 10, 20], "body: {result}");
}

// ---------------------------------------------------------------------------
// 演算子パラメータの検証
//
// クエリは遅延評価（`run_on_subset`）でしか実行されないため、そこで
// `validate()` を通していないと、範囲外パラメータを持つ演算子がセルを黙って
// 捨てて「エラーではなく空の結果 (200)」を返してしまう。以下はその退行を防ぐ。
// ---------------------------------------------------------------------------

/// `extrudeX` の座標がそのズームレベルの範囲外なら 400。
#[tokio::test]
async fn rejects_extrude_x_coordinate_out_of_range() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_ex_x", "Int", &[(760000, serde_json::json!(1))]).await;

    // z=5 の X 上限は 31。9999 は範囲外。
    let (status, body) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(760000, 1),
            "query": {
                "type": "extrudeX", "z": 5, "start": 0, "end": 9999, "policy": "max",
                "input": source("t_ex_x")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

/// `extrudeF` の高度がそのズームレベルの範囲外なら 400。
#[tokio::test]
async fn rejects_extrude_f_coordinate_out_of_range() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_ex_f", "Int", &[(761000, serde_json::json!(1))]).await;

    let (status, body) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(761000, 1),
            "query": {
                "type": "extrudeF", "z": 5, "start": 0, "end": 99999, "policy": "max",
                "input": source("t_ex_f")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

/// 値フィルタの下限が上限を上回っていたら 400。
#[tokio::test]
async fn rejects_inverted_filter_range() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_inv", "Int", &[(762000, serde_json::json!(5))]).await;

    let (status, body) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(762000, 1),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 100, "max": 1,
                "input": source("t_inv")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

/// 範囲内のパラメータなら従来どおり 200 で通ること（上の3件の裏取り）。
#[tokio::test]
async fn accepts_in_range_operator_parameters() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_ok", "Int", &[(763000, serde_json::json!(7))]).await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(763000, 1),
            "query": {
                "type": "filterValues", "mode": "inRange", "min": 1, "max": 100,
                "input": source("t_ok")
            }
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![7]);
    assert_eq!(total_ids(&result), 1);
}

// ---------------------------------------------------------------------------
// limit の挙動
// ---------------------------------------------------------------------------

/// `limit=0` で、値辞書に「どこからも参照されないエントリ」が残らないこと。
///
/// グループを組み立てる**前**に辞書へ push する実装だと、空間IDを1件も出力しないまま
/// 最初のグループの値だけが辞書に残る（`dictionary` に1件、`data` は空）。
/// 辞書長とグループ数が一致することで検出する。
#[tokio::test]
async fn zero_limit_leaves_no_orphan_dictionary_entries() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim",
        "Int",
        &[
            (770000, serde_json::json!(1)),
            (770001, serde_json::json!(2)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(770000, 2),
            "query": source("t_lim")
        }),
        "?limit=0",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(
        total_ids(&result),
        0,
        "limit=0 なのに空間IDが出ている: {result}"
    );

    let dict_len = result["dictionary"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let group_len = result["data"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(
        dict_len, 0,
        "参照されない辞書エントリが残っている: {result}"
    );
    assert_eq!(dict_len, group_len);
}

/// `limit` を掛けたときも、辞書エントリ数とグループ数が一致すること。
#[tokio::test]
async fn limit_keeps_dictionary_and_groups_consistent() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim1",
        "Int",
        &[
            (773000, serde_json::json!(1)),
            (773001, serde_json::json!(2)),
            (773002, serde_json::json!(3)),
            (773003, serde_json::json!(4)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(773000, 4),
            "query": source("t_lim1")
        }),
        "?limit=2",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 2, "limit が効いていない: {result}");

    let dict_len = result["dictionary"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let group_len = result["data"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(
        dict_len, group_len,
        "参照されない辞書エントリが残っている: {result}"
    );
}

/// `limit` での打ち切りは値の昇順で行われること。
///
/// 保持セル数を `limit` 件へ抑える最適化が、出力の中身を変えていないことの固定。
#[tokio::test]
async fn limit_keeps_the_smallest_values() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim2",
        "Int",
        &[
            (771000, serde_json::json!(40)),
            (771001, serde_json::json!(10)),
            (771002, serde_json::json!(30)),
            (771003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(771000, 4),
            "query": source("t_lim2")
        }),
        "?limit=2",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 2);
    assert_eq!(values(&result), vec![10, 20], "body: {result}");
}

/// `limit` 無指定なら全件返ること（上の2件の裏取り）。
#[tokio::test]
async fn no_limit_returns_every_cell() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim3",
        "Int",
        &[
            (772000, serde_json::json!(40)),
            (772001, serde_json::json!(10)),
            (772002, serde_json::json!(30)),
            (772003, serde_json::json!(20)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(772000, 4),
            "query": source("t_lim3")
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(total_ids(&result), 4);
    assert_eq!(values(&result), vec![10, 20, 30, 40]);
}

// ---------------------------------------------------------------------------
// value_type の明示
// ---------------------------------------------------------------------------

/// `Enum` テーブルを作る（`create_table` は制約を渡せないので直接リクエストする）。
async fn create_enum_table(app: &TestApp, table: &str, choices: &[&str]) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let body = serde_json::json!({
        "name": table,
        "data_type": "Enum",
        "constraints": { "type": "Enum", "choices": choices }
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

/// `data_type` が異なるソースは、既定では推論できず 400 になる。
///
/// `Text` と `Enum` はどちらも文字列として復元されるが、推論は `data_type` の
/// 同一性で判定するため弾かれる。
#[tokio::test]
async fn mixed_text_and_enum_sources_need_explicit_value_type() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_txt_m", "Text", &[(790000, serde_json::json!("a"))]).await;
    create_enum_table(&app, "t_enum_m", &["a", "b"]).await;
    put_data(
        &app,
        "t_enum_m",
        &serde_json::json!({ "value": "b", "spatial_ids": [single_id(790000)] }),
    )
    .await;

    let query = serde_json::json!({
        "type": "merge", "default": "", "policy": "max",
        "left": source("t_txt_m"),
        "right": source("t_enum_m")
    });

    let (status, _) = post_query(
        &app,
        &serde_json::json!({ "spatial_ids": ids(790000, 1), "query": query }),
        "?format=singleId",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "推論できてはいけない");
}

/// `value_type` を明示すれば、`Text` と `Enum` のソースを1つのクエリで混ぜられる。
///
/// 変換表を廃止したあと `value_type` に残る唯一の実用。
#[tokio::test]
async fn explicit_value_type_unifies_text_and_enum_sources() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_txt_u", "Text", &[(791000, serde_json::json!("a"))]).await;
    create_enum_table(&app, "t_enum_u", &["a", "b"]).await;
    put_data(
        &app,
        "t_enum_u",
        &serde_json::json!({ "value": "b", "spatial_ids": [single_id(791000)] }),
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            "spatial_ids": ids(791000, 1),
            "query": {
                "type": "merge", "default": "", "policy": "max",
                "left": source("t_txt_u"),
                "right": source("t_enum_u")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    // Enum 側は ID ではなく選択肢の文字列として読める。max("a", "b") = "b"
    assert_eq!(result["dictionary"][0], serde_json::json!("b"));
}

/// 指定した `value_type` として読めない `data_type` のソースがあれば 400。
#[tokio::test]
async fn explicit_value_type_rejects_unreadable_source() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_int_x", "Int", &[(792000, serde_json::json!(1))]).await;

    let (status, _) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            "spatial_ids": ids(792000, 1),
            "query": source("t_int_x")
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// 要求空間IDの解像度
//
// クエリ結果の解像度は演算子が決めるものであり、入力テーブルの最小セル
// とは別物。要求空間IDを丸めていた頃は、クエリ自身が生成したセルを指名できず 400 になっていた。
// ---------------------------------------------------------------------------

/// ズームレベルの絶対上限（30）は従来どおり検証される。
#[tokio::test]
async fn rejects_zoom_level_beyond_absolute_maximum() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    app.create_table("test_db", "t_zmax", "Int").await;

    let (status, _) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": [{ "z": 31, "f": 0, "x": 1, "y": 1, "type": "singleId" }],
            "query": source("t_zmax")
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}
