//! `QueryService::Execute` の統合テスト。
//!
//! クエリはシャードされた FlexTree を Kasane-Logic の入力源として読み、
//! 対象領域だけを遅延評価する。ここではその end-to-end の挙動を検証する。

mod routing;
mod values;

use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, merge, num, shift_x, source};
use crate::common::data::put_data;

/// `z=20, f=0, y=500000` 固定で X だけを振った空間ID。
fn single_id(x: i64) -> pb::SpatialId {
    builders::single_id(20, 0, x as u32, 500000)
}

/// 既定値（`format=singleId`, `value_type` 推論）のリクエストを組み立てる。
fn request(spatial_ids: Vec<pb::SpatialId>, query: pb::QueryNode) -> pb::ExecuteQueryRequest {
    pb::ExecuteQueryRequest {
        value_type: None,
        spatial_ids,
        query: Some(query),
        format: pb::OutputFormat::SingleId as i32,
    }
}

async fn execute_query(
    test_app: &TestApp,
    request: pb::ExecuteQueryRequest,
) -> Result<pb::SearchDataResponse, tonic::Status> {
    let stream = test_app.query().execute(request).await?.into_inner();
    Ok(crate::common::data::collect_search_stream(stream).await)
}

/// レスポンスに含まれる空間IDの総数。
fn total_ids(result: &pb::SearchDataResponse) -> usize {
    result.data.iter().map(|g| g.spatial_ids.len()).sum()
}

/// 出力に現れた値の集合を返す（昇順）。
fn values(result: &pb::SearchDataResponse) -> Vec<i64> {
    let mut out: Vec<i64> = result
        .data
        .iter()
        .map(|g| crate::common::data::group_value(&result.dictionary, g))
        .filter_map(builders::value_as_f64)
        .map(|n| n as i64)
        .collect();
    out.sort_unstable();
    out
}

/// 演算子を挟まない素通しクエリが、格納した値をそのまま返す。
#[tokio::test]
async fn query_source_only_returns_stored_values() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    put_data(&test_app, "test_table", num(42.0), vec![single_id(600000)]).await;

    let result = execute_query(
        &test_app,
        request(vec![single_id(600000)], source("test_db", "test_table")),
    )
    .await
    .unwrap();

    assert_eq!(total_ids(&result), 1);
    assert_eq!(values(&result), vec![42]);
}

/// shiftX で値が隣の FlexId へ移動する（元の位置には現れない）。
#[tokio::test]
async fn query_shift_x_moves_values() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    put_data(&test_app, "test_table", num(7.0), vec![single_id(610000)]).await;

    let query = shift_x(source("test_db", "test_table"), 20, 3);

    // 移動先には現れる
    let moved = execute_query(&test_app, request(vec![single_id(610003)], query.clone()))
        .await
        .unwrap();
    assert_eq!(values(&moved), vec![7]);

    // 元の位置には残らない
    let origin = execute_query(&test_app, request(vec![single_id(610000)], query))
        .await
        .unwrap();
    assert_eq!(total_ids(&origin), 0);
}

/// merge で2つのテーブル（別データベース）を1つのクエリで合成できる。
#[tokio::test]
async fn query_merge_across_two_databases() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;
    test_app.create_database("other_db").await;
    test_app
        .create_table("other_db", "other_table", "Int", 25)
        .await;

    // 同じ空間IDへ、別々のデータベースのテーブルから 10 と 5 を置く。
    put_data(&test_app, "test_table", num(10.0), vec![single_id(620000)]).await;

    test_app
        .data()
        .insert(pb::InsertDataRequest {
            db_name: "other_db".to_string(),
            table_name: "other_table".to_string(),
            value: Some(num(5.0)),
            spatial_ids: vec![single_id(620000)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .unwrap();

    let query = merge(
        source("test_db", "test_table"),
        source("other_db", "other_table"),
        num(0.0),
        pb::MergePolicyKind::Sum,
    );

    let result = execute_query(&test_app, request(vec![single_id(620000)], query))
        .await
        .unwrap();

    assert_eq!(values(&result), vec![15], "10 + 5 = 15 が返るはず");
}

/// ズームレベルの異なるテーブルを混在させても、上限は「最も細かいテーブル」に合わせる。
///
/// 細かいテーブル(zoom25)のズームレベルで問い合わせても弾かれず（旧実装は最も粗いテーブルに
/// 合わせていたため、デフォルトの `Error` policy で 400 になっていた）、
/// 粗いテーブル(zoom20)はその領域を内包する FlexId の値として正しく寄与する。
#[tokio::test]
async fn query_uses_finest_table_resolution() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "fine_table", "Int", 25)
        .await;
    test_app.create_database("coarse_db").await;
    test_app
        .create_table("coarse_db", "coarse_table", "Int", 20)
        .await;

    // 細かいテーブルへ zoom25 の FlexId へ 10 を置く。
    let fine_id = builders::single_id(25, 0, 620000, 500000);
    put_data(&test_app, "fine_table", num(10.0), vec![fine_id]).await;

    // 粗いテーブルへ、その zoom25 FlexId をちょうど内包する zoom20 の親 FlexId
    // (620000 >> 5 = 19375, 500000 >> 5 = 15625) へ 5 を置く。
    let coarse_id = builders::single_id(20, 0, 19375, 15625);
    test_app
        .data()
        .insert(pb::InsertDataRequest {
            db_name: "coarse_db".to_string(),
            table_name: "coarse_table".to_string(),
            value: Some(num(5.0)),
            spatial_ids: vec![coarse_id],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .unwrap();

    // zoom25 の空間IDで merge クエリを実行（policy は既定の Error 相当のことはしない、Sum を使う）。
    let query = merge(
        source("test_db", "fine_table"),
        source("coarse_db", "coarse_table"),
        num(0.0),
        pb::MergePolicyKind::Sum,
    );

    let result = execute_query(&test_app, request(vec![fine_id], query))
        .await
        .unwrap();

    // 細かいズームレベルで 10 + (内包する粗い FlexId の) 5 = 15 が返る。
    assert_eq!(values(&result), vec![15]);
}

/// data_type が異なるテーブルを混在させると InvalidArgument で拒否される。
#[tokio::test]
async fn query_rejects_mixed_data_types() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "int_table", "Int", 25)
        .await;
    test_app
        .create_table("test_db", "text_table", "Text", 25)
        .await;

    let query = merge(
        source("test_db", "int_table"),
        source("test_db", "text_table"),
        num(0.0),
        pb::MergePolicyKind::Sum,
    );

    let result = execute_query(&test_app, request(vec![single_id(630000)], query)).await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// 存在しないテーブルを参照すると NotFound。
#[tokio::test]
async fn query_missing_table_returns_404() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let result = execute_query(
        &test_app,
        request(vec![single_id(640000)], source("test_db", "nope")),
    )
    .await;

    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}
