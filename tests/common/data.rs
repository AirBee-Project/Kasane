use std::collections::HashMap;

use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{value_as_f64, value_as_str};

pub trait FromPbValue: Sized {
    fn from_pb_value(v: &prost_types::Value) -> Self;
}

impl FromPbValue for i64 {
    fn from_pb_value(v: &prost_types::Value) -> Self {
        value_as_f64(v).expect("expected a number") as i64
    }
}

impl FromPbValue for String {
    fn from_pb_value(v: &prost_types::Value) -> Self {
        value_as_str(v).expect("expected a string").to_string()
    }
}

impl FromPbValue for bool {
    fn from_pb_value(v: &prost_types::Value) -> Self {
        match &v.kind {
            Some(prost_types::value::Kind::BoolValue(b)) => *b,
            other => panic!("expected a bool, got {other:?}"),
        }
    }
}

/// `test_db` の `table_name` へデータを上書き挿入する（`Insert`）。
pub async fn put_data(
    test_app: &TestApp,
    table_name: &str,
    value: prost_types::Value,
    spatial_ids: Vec<pb::SpatialId>,
) {
    test_app
        .data()
        .insert(pb::InsertDataRequest {
            db_name: "test_db".to_string(),
            table_name: table_name.to_string(),
            value: Some(value),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .expect("insert failed");
}

/// `test_db` の `table_name` へデータを部分追加する（`Upsert`）。
pub async fn patch_data(
    test_app: &TestApp,
    table_name: &str,
    value: prost_types::Value,
    spatial_ids: Vec<pb::SpatialId>,
) {
    test_app
        .data()
        .upsert(pb::UpsertDataRequest {
            db_name: "test_db".to_string(),
            table_name: table_name.to_string(),
            value: Some(value),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .expect("upsert failed");
}

/// `test_db` の `table_name` を検索する（`format` は常に `singleId`）。
pub async fn search_data(
    test_app: &TestApp,
    table_name: &str,
    spatial_ids: Vec<pb::SpatialId>,
) -> pb::SearchDataResponse {
    test_app
        .data()
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: table_name.to_string(),
            spatial_ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::SingleId as i32,
            limit: None,
        })
        .await
        .expect("search failed")
        .into_inner()
}

/// `search_data` の結果の先頭エントリのデータ値と空間IDを検証する。
pub fn assert_first_entry(
    result: &pb::SearchDataResponse,
    expected_value: &prost_types::Value,
    expected_id: &pb::SingleId,
) {
    let group = result.data.first().expect("no data groups");
    let actual_value = result
        .dictionary
        .get(group.value_ref as usize)
        .expect("value_ref out of range");
    assert_eq!(actual_value, expected_value);

    let first_id = group.spatial_ids.first().expect("no spatial ids");
    match &first_id.kind {
        Some(pb::spatial_id::Kind::SingleId(s)) => {
            assert_eq!(s.z, expected_id.z);
            assert_eq!(s.f, expected_id.f);
            assert_eq!(s.x, expected_id.x);
            assert_eq!(s.y, expected_id.y);
        }
        other => panic!("expected a SingleId, got {other:?}"),
    }
}

/// `search_data` の結果を `(z, f, x, y) -> 値` の対応表に変換する。
///
/// `T` は `Int` レイヤーなら `i64`、`Text`/`Enum` レイヤーなら `String`、
/// `Boolean` レイヤーなら `bool` を指定する。
pub fn to_result_map<T: FromPbValue>(
    result: &pb::SearchDataResponse,
) -> HashMap<(u32, i32, u32, u32), T> {
    let mut map = HashMap::new();

    for group in &result.data {
        let Some(value) = result.dictionary.get(group.value_ref as usize) else {
            continue;
        };
        for id in &group.spatial_ids {
            if let Some(pb::spatial_id::Kind::SingleId(s)) = &id.kind {
                map.insert((s.z, s.f, s.x, s.y), T::from_pb_value(value));
            }
        }
    }

    map
}
