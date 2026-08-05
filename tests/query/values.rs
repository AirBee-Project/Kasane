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
    test_app.create_table("test_db", table, data_type, 25).await;
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
        "max_zoom_level": 25,
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
// クエリ結果の解像度は演算子が決めるものであり、入力テーブルの `max_zoom_level`
// （＝そのテーブルが保存する最小セル）とは別物。要求空間IDを `max_zoom_level` で
// 丸めていた頃は、クエリ自身が生成したセルを指名できず 400 になっていた。
// ---------------------------------------------------------------------------

/// `max_zoom_level` より細かい空間IDで、クエリが生成したセルを指名できる。
///
/// `max_zoom_level = 20` のテーブルに z=25 のサブセル shift を掛けると、結果は
/// z=21〜25 のセルを含む。それを z=25 の空間IDで取得できること。
#[tokio::test]
async fn accepts_targets_finer_than_source_max_zoom_level() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    app.create_table("test_db", "t_fine", "Int", 20).await;
    put_data(
        &app,
        "t_fine",
        &serde_json::json!({
            "value": 7,
            "spatial_ids": [{ "z": 20, "f": 0, "x": 800000, "y": 500000, "type": "singleId" }]
        }),
    )
    .await;

    // z=25 で 1 セル分ずらす（z=20 セルの 1/32）
    let query =
        serde_json::json!({ "type": "shiftX", "z": 25, "index": 1, "input": source("t_fine") });

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            // 元セル(z=20, x=800000) を z=25 へ落とすと x=25600000。shift 後は +1。
            "spatial_ids": [{ "z": 25, "f": 0, "x": 25600001, "y": 16000000, "type": "singleId" }],
            "query": query
        }),
        "?format=flexId",
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "max_zoom_level より細かい要求が弾かれている: {result}"
    );
    assert_eq!(values(&result), vec![7]);
    assert!(total_ids(&result) > 0, "セルが返っていない: {result}");
}

/// 粗い側（`max_zoom_level` 未満）の要求は従来どおり通る。
#[tokio::test]
async fn accepts_targets_coarser_than_source_max_zoom_level() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    app.create_table("test_db", "t_coarse", "Int", 20).await;
    put_data(
        &app,
        "t_coarse",
        &serde_json::json!({
            "value": 3,
            "spatial_ids": [{ "z": 20, "f": 0, "x": 800000, "y": 500000, "type": "singleId" }]
        }),
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": [{ "z": 18, "f": 0, "x": 200000, "y": 125000, "type": "singleId" }],
            "query": source("t_coarse")
        }),
        "?format=flexId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![3]);
}

/// ズームレベルの絶対上限（35）は従来どおり検証される。
#[tokio::test]
async fn rejects_zoom_level_beyond_absolute_maximum() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    app.create_table("test_db", "t_zmax", "Int", 20).await;

    let (status, _) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": [{ "z": 40, "f": 0, "x": 1, "y": 1, "type": "singleId" }],
            "query": source("t_zmax")
        }),
        "",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Text を Int へ変換し、対応表に無い値は `default` になる。
#[tokio::test]
async fn map_values_converts_types_and_applies_fallback() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map",
        "Text",
        &[
            (800000, serde_json::json!("Sunny")),
            (800001, serde_json::json!("Cloudy")),
            (800002, serde_json::json!("Rainy")),
            (800003, serde_json::json!("Unknown")),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(800000, 4),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": "Sunny", "to": 100 },
                    { "from": "Cloudy", "to": 50 },
                    { "from": "Rainy", "to": 0 }
                ],
                "default": -1,
                "input": source("t_map")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    // "Unknown" だけが対応表に無いので -1 になる。
    assert_eq!(values(&result), vec![-1, 0, 50, 100]);
    assert_eq!(total_ids(&result), 4);
}

/// Int を Text へ変換する。
#[tokio::test]
async fn map_values_converts_int_to_text() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_int",
        "Int",
        &[
            (801000, serde_json::json!(1)),
            (801001, serde_json::json!(2)),
            (801002, serde_json::json!(3)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            "spatial_ids": ids(801000, 3),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": 1, "to": "One" },
                    { "from": 2, "to": "Two" }
                ],
                "default": "Other",
                "input": source("t_map_int")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    // 辞書は値の昇順。3 は対応表に無いので "Other" になる。
    assert_eq!(
        result["dictionary"],
        serde_json::json!(["One", "Other", "Two"])
    );
    assert_eq!(total_ids(&result), 3);
}

/// Boolean を Int へ変換する。
#[tokio::test]
async fn map_values_converts_boolean_to_int() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_bool",
        "Boolean",
        &[
            (802000, serde_json::json!(true)),
            (802001, serde_json::json!(false)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(802000, 2),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": true, "to": 1 },
                    { "from": false, "to": 0 }
                ],
                "default": -1,
                "input": source("t_map_bool")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![0, 1]);
    assert_eq!(total_ids(&result), 2);
}

/// Float を Boolean へ変換する。
#[tokio::test]
async fn map_values_converts_float_to_boolean() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_float",
        "Float",
        &[
            (803000, serde_json::json!(1.5)),
            (803001, serde_json::json!(2.5)),
            (803002, serde_json::json!(3.5)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Boolean",
            "spatial_ids": ids(803000, 3),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": 1.5, "to": true },
                    { "from": 2.5, "to": false }
                ],
                "default": true,
                "input": source("t_map_float")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    // values() は Int/Float 専用なので辞書を直接見る。3.5 は対応表に無く default の true。
    assert_eq!(result["dictionary"], serde_json::json!([false, true]));
    assert_eq!(total_ids(&result), 3);
}

/// mapValues が結果の値型を決める位置にあるとき、`value_type` の省略は 400。
#[tokio::test]
async fn map_values_rejects_missing_value_type() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_missing_vt",
        "Int",
        &[(804000, serde_json::json!(1))],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "spatial_ids": ids(804000, 1),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": 1, "to": "One" }
                ],
                "default": "Other",
                "input": source("t_map_missing_vt")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("cannot infer the query value type because it contains a mapValues operator"),
        "error: {result}"
    );
}

/// merge の両辺が mapValues でも、リクエストの `value_type` が両方の出力型になる。
#[tokio::test]
async fn map_values_as_both_merge_operands() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_merge",
        "Int",
        &[(805000, serde_json::json!(1))],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            "spatial_ids": ids(805000, 1),
            "query": {
                "type": "merge",
                "policy": "min",
                "default": "Z",
                "left": {
                    "type": "mapValues",
                    "mapping": [{ "from": 1, "to": "A" }],
                    "default": "Z",
                    "input": source("t_map_merge")
                },
                "right": {
                    "type": "mapValues",
                    "mapping": [{ "from": 1, "to": "B" }],
                    "default": "Z",
                    "input": source("t_map_merge")
                }
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(result["dictionary"], serde_json::json!(["A"]));
    assert_eq!(total_ids(&result), 1);
}

/// mapValues を直接入れ子にする場合、内側の出力型は推論できないので
/// 外側の `input_type` で明示する必要がある。
#[tokio::test]
async fn map_values_nested_requires_input_type() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_map_nest", "Int", &[(805100, serde_json::json!(1))]).await;

    let inner = serde_json::json!({
        "type": "mapValues",
        "mapping": [{ "from": 1, "to": "One" }],
        "default": "X",
        "input": source("t_map_nest")
    });

    // 外側が内側の出力型を推論できず 400。
    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(805100, 1),
            "query": {
                "type": "mapValues",
                "mapping": [{ "from": "One", "to": 11 }],
                "default": -1,
                "input": inner
            }
        }),
        "?format=singleId",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("mapValues operator"),
        "error: {result}"
    );

    // `input_type` を明示すれば通る。
    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(805100, 1),
            "query": {
                "type": "mapValues",
                "input_type": "Text",
                "mapping": [{ "from": "One", "to": 11 }],
                "default": -1,
                "input": inner
            }
        }),
        "?format=singleId",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![11]);
}

/// `from` が重複した対応表は 400 で拒否される。
#[tokio::test]
async fn map_values_rejects_duplicate_mapping_keys() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(&app, "t_map_dup", "Int", &[(806000, serde_json::json!(1))]).await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            "spatial_ids": ids(806000, 1),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": 1, "to": "One" },
                    { "from": 1, "to": "Uno" }
                ],
                "default": "Other",
                "input": source("t_map_dup")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("duplicate mapping key"),
        "error: {result}"
    );
}

/// 空の対応表は、全ての値を `default` に潰す。
#[tokio::test]
async fn map_values_allows_empty_mapping() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_empty",
        "Int",
        &[(807000, serde_json::json!(1))],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            "spatial_ids": ids(807000, 1),
            "query": {
                "type": "mapValues",
                "mapping": [],
                "default": "Other",
                "input": source("t_map_empty")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(result["dictionary"], serde_json::json!(["Other"]));
    assert_eq!(total_ids(&result), 1);
}

/// ソースの型として読めない `input_type` は拒否される。
#[tokio::test]
async fn map_values_rejects_wrong_input_type() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_wrong_input",
        "Text",
        &[(808000, serde_json::json!("A"))],
    )
    .await;

    // ソースは Text なのに、input_type で Int だと主張している。
    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            "spatial_ids": ids(808000, 1),
            "query": {
                "type": "mapValues",
                "input_type": "Int",
                "mapping": [
                    { "from": 1, "to": "One" }
                ],
                "default": "Other",
                "input": source("t_map_wrong_input")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("cannot be read as the query value type Int"),
        "error: {result}"
    );
}

/// Float の対応表で `-0.0` と `0.0` が同一視される。
///
/// `OrderedFloat` の順序は `total_cmp` なので、正規化しないと `-0.0` を書いた
/// エントリが `0.0` に一致せず、重複としても検出されない。
#[tokio::test]
async fn map_values_treats_negative_zero_as_zero() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_negzero",
        "Float",
        &[(811000, serde_json::json!(0.0))],
    )
    .await;

    // `-0.0` のエントリが、格納された `0.0` に一致する。
    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(811000, 1),
            "query": {
                "type": "mapValues",
                "mapping": [{ "from": -0.0, "to": 7 }],
                "default": -1,
                "input": source("t_map_negzero")
            }
        }),
        "?format=singleId",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![7]);

    // 同じ理由で `0.0` と `-0.0` の併記は重複として弾かれる。
    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(811000, 1),
            "query": {
                "type": "mapValues",
                "mapping": [{ "from": 0.0, "to": 7 }, { "from": -0.0, "to": 8 }],
                "default": -1,
                "input": source("t_map_negzero")
            }
        }),
        "?format=singleId",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("duplicate mapping key"),
        "error: {result}"
    );
}

/// Enum ソースは選択肢の文字列として対応表に照合される。
#[tokio::test]
async fn map_values_supports_enum_source() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    create_enum_table(&app, "t_map_enum", &["A", "B"]).await;
    put_data(
        &app,
        "t_map_enum",
        &serde_json::json!({ "value": "A", "spatial_ids": [single_id(809000)] }),
    )
    .await;
    put_data(
        &app,
        "t_map_enum",
        &serde_json::json!({ "value": "B", "spatial_ids": [single_id(809001)] }),
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Int",
            "spatial_ids": ids(809000, 2),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": "A", "to": 10 },
                    { "from": "B", "to": 20 }
                ],
                "default": 0,
                "input": source("t_map_enum")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    assert_eq!(values(&result), vec![10, 20]);
    assert_eq!(total_ids(&result), 2);
}

/// 値の無いセルは `default` にならず、欠損のまま残る。
#[tokio::test]
async fn map_values_keeps_missing_cells() {
    let app = TestApp::new();
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_missing_cell",
        "Int",
        &[
            (810000, serde_json::json!(1)),
            // 810001 には値を置かない
            (810002, serde_json::json!(3)),
        ],
    )
    .await;

    let (status, result) = post_query(
        &app,
        &serde_json::json!({
            "value_type": "Text",
            // 3 件問い合わせるが、ソースにあるのは 2 件だけ。
            "spatial_ids": ids(810000, 3),
            "query": {
                "type": "mapValues",
                "mapping": [
                    { "from": 1, "to": "One" }
                ],
                "default": "Other",
                "input": source("t_map_missing_cell")
            }
        }),
        "?format=singleId",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {result}");
    // 1 -> "One"、3 -> default の "Other"。空の 810001 は "Other" にならない。
    assert_eq!(result["dictionary"], serde_json::json!(["One", "Other"]));
    assert_eq!(total_ids(&result), 2);
}
