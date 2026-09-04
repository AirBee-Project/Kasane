//! 値に注目したクエリ（値フィルタ）と、全データ型への対応の検証。

use kasane::grpc::pb;

use super::{execute_query, request, single_id, total_ids, values};
use crate::common::TestApp;
use crate::common::builders::{
    self, boolean, extrude_f, extrude_x, falloff_x, filter_equals, filter_in_range,
    filter_not_in_range, map_values, mapping_entry, merge, num, shift_x, text, zoom_out,
};
use crate::common::data::put_data;

/// `test_db` に指定型のテーブルを作り、`(x, 値)` を書き込む。
async fn seed(
    test_app: &TestApp,
    table: &str,
    data_type: &str,
    flex_ids: &[(i64, pb::TypedValue)],
) {
    test_app.create_table("test_db", table, data_type, 25).await;
    for (x, v) in flex_ids {
        put_data(test_app, table, v.clone(), vec![single_id(*x)]).await;
    }
}

fn source(table: &str) -> pb::QueryNode {
    builders::source("test_db", table)
}

fn ids(base: i64, count: i64) -> Vec<pb::SpatialId> {
    (0..count).map(|i| single_id(base + i)).collect()
}

fn value_as_bool(v: &pb::TypedValue) -> Option<bool> {
    match &v.kind {
        Some(pb::typed_value::Kind::BoolVal(b)) => Some(*b),
        _ => None,
    }
}

fn dictionary_strings(result: &pb::SearchDataResponse) -> Vec<&str> {
    result
        .dictionary
        .iter()
        .map(|v| builders::value_as_str(v).expect("expected a string"))
        .collect()
}

/// `test_db` に `Enum` テーブルを作る（`create_table` は制約を渡せないので直接リクエストする）。
async fn create_enum_table(app: &TestApp, table: &str, choices: &[&str]) {
    app.table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: table.to_string(),
            data_type: pb::TableDataType::Enum as i32,
            max_zoom_level: 25,
            constraints: Some(pb::TableConstraints {
                kind: Some(pb::table_constraints::Kind::EnumConstraint(
                    pb::table_constraints::Enum {
                        choices: choices.iter().map(|s| s.to_string()).collect(),
                    },
                )),
            }),
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .unwrap();
}

/// ある値のみを残す。
#[tokio::test]
async fn filter_equals_keeps_only_that_value() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_eq",
        "Int",
        &[(700000, num(1.0)), (700001, num(5.0)), (700002, num(10.0))],
    )
    .await;

    let query = filter_equals(source("t_eq"), num(5.0));
    let result = execute_query(&app, request(ids(700000, 3), query))
        .await
        .unwrap();

    assert_eq!(values(&result), vec![5]);
    assert_eq!(total_ids(&result), 1);
}

/// ある範囲の値を残す（閉区間）。
#[tokio::test]
async fn filter_in_range_is_inclusive() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_in",
        "Int",
        &[
            (710000, num(1.0)),
            (710001, num(5.0)),
            (710002, num(10.0)),
            (710003, num(20.0)),
        ],
    )
    .await;

    let query = filter_in_range(source("t_in"), Some(num(5.0)), Some(num(10.0)));
    let result = execute_query(&app, request(ids(710000, 4), query))
        .await
        .unwrap();

    assert_eq!(values(&result), vec![5, 10], "境界を含むこと");
}

/// ある範囲の値**以外**を残す。
#[tokio::test]
async fn filter_not_in_range_keeps_the_outside() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_out",
        "Int",
        &[
            (720000, num(1.0)),
            (720001, num(5.0)),
            (720002, num(10.0)),
            (720003, num(20.0)),
        ],
    )
    .await;

    let query = filter_not_in_range(source("t_out"), Some(num(5.0)), Some(num(10.0)));
    let result = execute_query(&app, request(ids(720000, 4), query))
        .await
        .unwrap();

    assert_eq!(values(&result), vec![1, 20]);
}

/// Text テーブルでも値フィルタは使える（比較に必要なのは順序だけ）。
#[tokio::test]
async fn filter_works_on_text_values() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_words",
        "Text",
        &[
            (760000, text("apple")),
            (760001, text("banana")),
            (760002, text("cherry")),
        ],
    )
    .await;

    let query = filter_equals(source("t_words"), text("banana"));
    let result = execute_query(&app, request(ids(760000, 3), query))
        .await
        .unwrap();

    assert_eq!(total_ids(&result), 1);
    assert_eq!(
        builders::value_as_str(&result.dictionary[0]),
        Some("banana")
    );
}

/// Boolean テーブルにもクエリを適用できる。
#[tokio::test]
async fn supports_boolean_tables() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_flag",
        "Boolean",
        &[(770000, boolean(true)), (770001, boolean(false))],
    )
    .await;

    let query = filter_equals(source("t_flag"), boolean(true));
    let result = execute_query(&app, request(ids(770000, 2), query))
        .await
        .unwrap();

    assert_eq!(total_ids(&result), 1);
    assert_eq!(value_as_bool(&result.dictionary[0]), Some(true));
}

/// 64bit 相当の大きな整数値にもクエリを適用できる（`Int` = i64）。
#[tokio::test]
async fn supports_large_int_values() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_big",
        "Int",
        &[(778000, num(1_000_000_000_000.0)), (778001, num(1.0))],
    )
    .await;

    let query = filter_in_range(source("t_big"), Some(num(1_000_000.0)), None);
    let result = execute_query(&app, request(ids(778000, 2), query))
        .await
        .unwrap();

    assert_eq!(total_ids(&result), 1);
    assert_eq!(
        builders::value_as_f64(&result.dictionary[0]),
        Some(1_000_000_000_000.0)
    );
}

/// 算術を要する演算子は非数値型では InvalidArgument になる。
#[tokio::test]
async fn rejects_arithmetic_operator_on_text() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_txt3", "Text", &[]).await;

    let query = falloff_x(
        source("t_txt3"),
        20,
        2,
        pb::FalloffPattern::Linear,
        None,
        pb::MergePolicyKind::Max,
    );
    let result = execute_query(&app, request(ids(780000, 1), query)).await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// 数値専用の `MergePolicy` を非数値型に使うと InvalidArgument になる。
#[tokio::test]
async fn rejects_numeric_policy_on_text() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_txt4", "Text", &[]).await;

    let query = zoom_out(source("t_txt4"), 19, pb::MergePolicyKind::Sum);
    let result = execute_query(&app, request(ids(785000, 1), query)).await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// 演算子を挟まないクエリで、対象領域の全 FlexId が返ること。
///
/// 対象領域の求め方を誤ると一部の FlexId だけが返る退行が起きるため、複数 FlexId で固定する。
#[tokio::test]
async fn source_only_returns_all_flex_ids() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_multi",
        "Int",
        &[
            (790000, num(1.0)),
            (790001, num(5.0)),
            (790002, num(10.0)),
            (790003, num(20.0)),
        ],
    )
    .await;

    let result = execute_query(&app, request(ids(790000, 4), source("t_multi")))
        .await
        .unwrap();

    assert_eq!(values(&result), vec![1, 5, 10, 20]);
}

// ---------------------------------------------------------------------------
// 演算子パラメータの検証
//
// クエリは遅延評価（`run_on_subset`）でしか実行されないため、そこで
// `validate()` を通していないと、範囲外パラメータを持つ演算子が FlexId を黙って
// 捨てて「エラーではなく空の結果」を返してしまう。以下はその退行を防ぐ。
// ---------------------------------------------------------------------------

/// `extrudeX` の座標がそのズームレベルの範囲外なら InvalidArgument。
#[tokio::test]
async fn rejects_extrude_x_coordinate_out_of_range() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_ex_x", "Int", &[(760000, num(1.0))]).await;

    // z=5 の X 上限は 31。9999 は範囲外。
    let query = extrude_x(source("t_ex_x"), 5, 0, 9999, pb::MergePolicyKind::Max);
    let result = execute_query(&app, request(ids(760000, 1), query)).await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// `extrudeF` の高度がそのズームレベルの範囲外なら InvalidArgument。
#[tokio::test]
async fn rejects_extrude_f_coordinate_out_of_range() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_ex_f", "Int", &[(761000, num(1.0))]).await;

    let query = extrude_f(source("t_ex_f"), 5, 0, 99999, pb::MergePolicyKind::Max);
    let result = execute_query(&app, request(ids(761000, 1), query)).await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// 値フィルタの下限が上限を上回っていたら InvalidArgument。
#[tokio::test]
async fn rejects_inverted_filter_range() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_inv", "Int", &[(762000, num(5.0))]).await;

    let query = filter_in_range(source("t_inv"), Some(num(100.0)), Some(num(1.0)));
    let result = execute_query(&app, request(ids(762000, 1), query)).await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// 範囲内のパラメータなら従来どおり通ること（上の3件の裏取り）。
#[tokio::test]
async fn accepts_in_range_operator_parameters() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_ok", "Int", &[(763000, num(7.0))]).await;

    let query = filter_in_range(source("t_ok"), Some(num(1.0)), Some(num(100.0)));
    let result = execute_query(&app, request(ids(763000, 1), query))
        .await
        .unwrap();

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
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim",
        "Int",
        &[(770000, num(1.0)), (770001, num(2.0))],
    )
    .await;

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            limit: Some(0),
            ..request(ids(770000, 2), source("t_lim"))
        },
    )
    .await
    .unwrap();

    assert_eq!(total_ids(&result), 0, "limit=0 なのに空間IDが出ている");
    assert_eq!(
        result.dictionary.len(),
        0,
        "参照されない辞書エントリが残っている"
    );
    assert_eq!(result.dictionary.len(), result.data.len());
}

/// `limit` を掛けたときも、辞書エントリ数とグループ数が一致すること。
#[tokio::test]
async fn limit_keeps_dictionary_and_groups_consistent() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim1",
        "Int",
        &[
            (773000, num(1.0)),
            (773001, num(2.0)),
            (773002, num(3.0)),
            (773003, num(4.0)),
        ],
    )
    .await;

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            limit: Some(2),
            ..request(ids(773000, 4), source("t_lim1"))
        },
    )
    .await
    .unwrap();

    assert_eq!(total_ids(&result), 2, "limit が効いていない");
    assert_eq!(
        result.dictionary.len(),
        result.data.len(),
        "参照されない辞書エントリが残っている"
    );
}

/// `limit` での打ち切りは順序を問わず行われること。
///
/// 保持 FlexId 数を `limit` 件へ抑える短絡評価により、上位 `limit` 件が保証されるわけではないことの確認。
#[tokio::test]
async fn limit_truncates_the_results() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim2",
        "Int",
        &[
            (771000, num(40.0)),
            (771001, num(10.0)),
            (771002, num(30.0)),
            (771003, num(20.0)),
        ],
    )
    .await;

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            limit: Some(2),
            ..request(ids(771000, 4), source("t_lim2"))
        },
    )
    .await
    .unwrap();

    assert_eq!(total_ids(&result), 2);
    // どの値が返るかは基盤ストレージの走査順に依存するため、件数だけを検証する。
    assert_eq!(values(&result).len(), 2);
}

/// `limit` 無指定なら全件返ること（上の2件の裏取り）。
#[tokio::test]
async fn no_limit_returns_every_flex_id() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_lim3",
        "Int",
        &[
            (772000, num(40.0)),
            (772001, num(10.0)),
            (772002, num(30.0)),
            (772003, num(20.0)),
        ],
    )
    .await;

    let result = execute_query(&app, request(ids(772000, 4), source("t_lim3")))
        .await
        .unwrap();

    assert_eq!(total_ids(&result), 4);
    assert_eq!(values(&result), vec![10, 20, 30, 40]);
}

// ---------------------------------------------------------------------------
// value_type の明示
// ---------------------------------------------------------------------------

/// `data_type` が異なるソースは、既定では推論できず InvalidArgument になる。
///
/// `Text` と `Enum` はどちらも文字列として復元されるが、推論は `data_type` の
/// 同一性で判定するため弾かれる。
#[tokio::test]
async fn mixed_text_and_enum_sources_need_explicit_value_type() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_txt_m", "Text", &[(790000, text("a"))]).await;
    create_enum_table(&app, "t_enum_m", &["a", "b"]).await;
    put_data(&app, "t_enum_m", text("b"), vec![single_id(790000)]).await;

    let query = merge(
        source("t_txt_m"),
        source("t_enum_m"),
        text(""),
        pb::MergePolicyKind::Max,
    );

    let result = execute_query(&app, request(ids(790000, 1), query)).await;
    assert_eq!(
        result.unwrap_err().code(),
        tonic::Code::InvalidArgument,
        "推論できてはいけない"
    );
}

/// `value_type` を明示すれば、`Text` と `Enum` のソースを1つのクエリで混ぜられる。
///
/// 変換表を廃止したあと `value_type` に残る唯一の実用。
#[tokio::test]
async fn explicit_value_type_unifies_text_and_enum_sources() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_txt_u", "Text", &[(791000, text("a"))]).await;
    create_enum_table(&app, "t_enum_u", &["a", "b"]).await;
    put_data(&app, "t_enum_u", text("b"), vec![single_id(791000)]).await;

    let query = merge(
        source("t_txt_u"),
        source("t_enum_u"),
        text(""),
        pb::MergePolicyKind::Max,
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            ..request(ids(791000, 1), query)
        },
    )
    .await
    .unwrap();

    // Enum 側は ID ではなく選択肢の文字列として読める。max("a", "b") = "b"
    assert_eq!(builders::value_as_str(&result.dictionary[0]), Some("b"));
}

/// 指定した `value_type` として読めない `data_type` のソースがあれば InvalidArgument。
#[tokio::test]
async fn explicit_value_type_rejects_unreadable_source() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_int_x", "Int", &[(792000, num(1.0))]).await;

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            ..request(ids(792000, 1), source("t_int_x"))
        },
    )
    .await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

// ---------------------------------------------------------------------------
// 要求空間IDのズームレベル
//
// クエリ結果の解像度は演算子が決めるものであり、入力テーブルの `max_zoom_level`
// （＝そのテーブルが保存する最小 FlexId ）とは別物。要求空間IDを `max_zoom_level` で
// 丸めていた頃は、クエリ自身が生成した FlexId を指名できず InvalidArgument になっていた。
// ---------------------------------------------------------------------------

/// `max_zoom_level` より細かい空間IDで、クエリが生成した FlexId を指名できる。
///
/// `max_zoom_level = 20` のテーブルに z=25 のサブ FlexId shift を掛けると、結果は
/// z=21〜25 の FlexId を含む。それを z=25 の空間IDで取得できること。
#[tokio::test]
async fn accepts_targets_finer_than_source_max_zoom_level() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    app.create_table("test_db", "t_fine", "Int", 20).await;
    put_data(
        &app,
        "t_fine",
        num(7.0),
        vec![builders::single_id(20, 0, 800000, 500000)],
    )
    .await;

    // z=25 で 1 FlexId 分ずらす（z=20 FlexId の 1/32）
    let query = shift_x(source("t_fine"), 25, 1);

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            // 元の FlexId(z=20, x=800000) を z=25 へ落とすと x=25600000。shift 後は +1。
            spatial_ids: vec![builders::single_id(25, 0, 25600001, 16000000)],
            format: pb::OutputFormat::FlexId as i32,
            ..request(vec![], query)
        },
    )
    .await
    .unwrap();

    assert_eq!(values(&result), vec![7]);
    assert!(total_ids(&result) > 0, "FlexId が返っていない");
}

/// 粗い側（`max_zoom_level` 未満）の要求は従来どおり通る。
#[tokio::test]
async fn accepts_targets_coarser_than_source_max_zoom_level() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    app.create_table("test_db", "t_coarse", "Int", 20).await;
    put_data(
        &app,
        "t_coarse",
        num(3.0),
        vec![builders::single_id(20, 0, 800000, 500000)],
    )
    .await;

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            spatial_ids: vec![builders::single_id(18, 0, 200000, 125000)],
            format: pb::OutputFormat::FlexId as i32,
            ..request(vec![], source("t_coarse"))
        },
    )
    .await
    .unwrap();

    assert_eq!(values(&result), vec![3]);
}

/// ズームレベルの絶対上限（35）は従来どおり検証される。
#[tokio::test]
async fn rejects_zoom_level_beyond_absolute_maximum() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    app.create_table("test_db", "t_zmax", "Int", 20).await;

    let result = execute_query(
        &app,
        request(vec![builders::single_id(40, 0, 1, 1)], source("t_zmax")),
    )
    .await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// Text を Int へ変換し、対応表に無い値は `default` になる。
#[tokio::test]
async fn map_values_converts_types_and_applies_fallback() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map",
        "Text",
        &[
            (800000, text("Sunny")),
            (800001, text("Cloudy")),
            (800002, text("Rainy")),
            (800003, text("Unknown")),
        ],
    )
    .await;

    let mapping = vec![
        mapping_entry(text("Sunny"), num(100.0)),
        mapping_entry(text("Cloudy"), num(50.0)),
        mapping_entry(text("Rainy"), num(0.0)),
    ];
    let query = map_values(source("t_map"), pb::TableDataType::Int, mapping, num(-1.0));

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Int as i32),
            ..request(ids(800000, 4), query)
        },
    )
    .await
    .unwrap();

    // "Unknown" だけが対応表に無いので -1 になる。
    assert_eq!(values(&result), vec![-1, 0, 50, 100]);
    assert_eq!(total_ids(&result), 4);
}

/// Int を Text へ変換する。
#[tokio::test]
async fn map_values_converts_int_to_text() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_int",
        "Int",
        &[(801000, num(1.0)), (801001, num(2.0)), (801002, num(3.0))],
    )
    .await;

    let mapping = vec![
        mapping_entry(num(1.0), text("One")),
        mapping_entry(num(2.0), text("Two")),
    ];
    let query = map_values(
        source("t_map_int"),
        pb::TableDataType::Text,
        mapping,
        text("Other"),
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            ..request(ids(801000, 3), query)
        },
    )
    .await
    .unwrap();

    // 辞書は値の昇順。3 は対応表に無いので "Other" になる。
    assert_eq!(dictionary_strings(&result), vec!["One", "Other", "Two"]);
    assert_eq!(total_ids(&result), 3);
}

/// Boolean を Int へ変換する。
#[tokio::test]
async fn map_values_converts_boolean_to_int() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_bool",
        "Boolean",
        &[(802000, boolean(true)), (802001, boolean(false))],
    )
    .await;

    let mapping = vec![
        mapping_entry(boolean(true), num(1.0)),
        mapping_entry(boolean(false), num(0.0)),
    ];
    let query = map_values(
        source("t_map_bool"),
        pb::TableDataType::Int,
        mapping,
        num(-1.0),
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Int as i32),
            ..request(ids(802000, 2), query)
        },
    )
    .await
    .unwrap();

    assert_eq!(values(&result), vec![0, 1]);
    assert_eq!(total_ids(&result), 2);
}

/// mapValues が `output_type` を持つため、リクエストの `value_type` を省略しても正常に推論される。
#[tokio::test]
async fn map_values_infers_type_from_output_type() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_map_missing_vt", "Int", &[(804000, num(1.0))]).await;

    let mapping = vec![mapping_entry(num(1.0), text("One"))];
    let query = map_values(
        source("t_map_missing_vt"),
        pb::TableDataType::Text,
        mapping,
        text("Other"),
    );

    let result = execute_query(&app, request(ids(804000, 1), query))
        .await
        .unwrap();

    assert_eq!(dictionary_strings(&result), vec!["One"]);
    assert_eq!(total_ids(&result), 1);
}

/// merge の両辺が mapValues でも、リクエストの `value_type` が両方の出力型になる。
#[tokio::test]
async fn map_values_as_both_merge_operands() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_map_merge", "Int", &[(805000, num(1.0))]).await;

    let left = map_values(
        source("t_map_merge"),
        pb::TableDataType::Text,
        vec![mapping_entry(num(1.0), text("A"))],
        text("Z"),
    );
    let right = map_values(
        source("t_map_merge"),
        pb::TableDataType::Text,
        vec![mapping_entry(num(1.0), text("B"))],
        text("Z"),
    );
    let query = merge(left, right, text("Z"), pb::MergePolicyKind::Min);

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            ..request(ids(805000, 1), query)
        },
    )
    .await
    .unwrap();

    assert_eq!(dictionary_strings(&result), vec!["A"]);
    assert_eq!(total_ids(&result), 1);
}

/// mapValues を直接入れ子にする場合、内側の出力型は外側の入力型として自動解決される。
#[tokio::test]
async fn map_values_nested_infers_input_type_automatically() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_map_nest", "Int", &[(805100, num(1.0))]).await;

    let inner = map_values(
        source("t_map_nest"),
        pb::TableDataType::Text,
        vec![mapping_entry(num(1.0), text("One"))],
        text("X"),
    );
    let query = map_values(
        inner,
        pb::TableDataType::Int,
        vec![mapping_entry(text("One"), num(11.0))],
        num(-1.0),
    );

    let result = execute_query(&app, request(ids(805100, 1), query))
        .await
        .unwrap();
    assert_eq!(values(&result), vec![11]);
}

/// `from` が重複した対応表は InvalidArgument で拒否される。
#[tokio::test]
async fn map_values_rejects_duplicate_mapping_keys() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_map_dup", "Int", &[(806000, num(1.0))]).await;

    let mapping = vec![
        mapping_entry(num(1.0), text("One")),
        mapping_entry(num(1.0), text("Uno")),
    ];
    let query = map_values(
        source("t_map_dup"),
        pb::TableDataType::Text,
        mapping,
        text("Other"),
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            ..request(ids(806000, 1), query)
        },
    )
    .await;

    let err = result.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message().contains("duplicate mapping key"),
        "message: {}",
        err.message()
    );
}

/// 空の対応表は、全ての値を `default` に潰す。
#[tokio::test]
async fn map_values_allows_empty_mapping() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_map_empty", "Int", &[(807000, num(1.0))]).await;

    let query = map_values(
        source("t_map_empty"),
        pb::TableDataType::Text,
        vec![],
        text("Other"),
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            ..request(ids(807000, 1), query)
        },
    )
    .await
    .unwrap();

    assert_eq!(dictionary_strings(&result), vec!["Other"]);
    assert_eq!(total_ids(&result), 1);
}

/// Enum ソースは選択肢の文字列として対応表に照合される。
#[tokio::test]
async fn map_values_supports_enum_source() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    create_enum_table(&app, "t_map_enum", &["A", "B"]).await;
    put_data(&app, "t_map_enum", text("A"), vec![single_id(809000)]).await;
    put_data(&app, "t_map_enum", text("B"), vec![single_id(809001)]).await;

    let mapping = vec![
        mapping_entry(text("A"), num(10.0)),
        mapping_entry(text("B"), num(20.0)),
    ];
    let query = map_values(
        source("t_map_enum"),
        pb::TableDataType::Int,
        mapping,
        num(0.0),
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Int as i32),
            ..request(ids(809000, 2), query)
        },
    )
    .await
    .unwrap();

    assert_eq!(values(&result), vec![10, 20]);
    assert_eq!(total_ids(&result), 2);
}

/// 値の無い FlexId は `default` にならず、欠損のまま残る。
#[tokio::test]
async fn map_values_keeps_missing_flex_ids() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(
        &app,
        "t_map_missing_flex_id",
        "Int",
        &[
            (810000, num(1.0)),
            // 810001 には値を置かない
            (810002, num(3.0)),
        ],
    )
    .await;

    let query = map_values(
        source("t_map_missing_flex_id"),
        pb::TableDataType::Text,
        vec![mapping_entry(num(1.0), text("One"))],
        text("Other"),
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            // 3 件問い合わせるが、ソースにあるのは 2 件だけ。
            ..request(ids(810000, 3), query)
        },
    )
    .await
    .unwrap();

    // 1 -> "One"、3 -> default の "Other"。空の 810001 は "Other" にならない。
    assert_eq!(dictionary_strings(&result), vec!["One", "Other"]);
    assert_eq!(total_ids(&result), 2);
}

/// `output_type` がリクエストの `value_type` と食い違う場合は InvalidArgument で拒否される
/// （`output_type` は宣言だけでなく、実際に使われる値型と一致することを検証される）。
#[tokio::test]
async fn map_values_rejects_output_type_mismatch() {
    let app = TestApp::new().await;
    app.create_database("test_db").await;
    seed(&app, "t_map_output_mismatch", "Int", &[(812000, num(1.0))]).await;

    // クエリ全体の値型は Text だが、mapValues の output_type は Int だと主張している。
    let query = map_values(
        source("t_map_output_mismatch"),
        pb::TableDataType::Int,
        vec![mapping_entry(num(1.0), text("5"))],
        text("-1"),
    );

    let result = execute_query(
        &app,
        pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Text as i32),
            ..request(ids(812000, 1), query)
        },
    )
    .await;

    let err = result.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message().contains("mapValues output_type"),
        "message: {}",
        err.message()
    );
}
