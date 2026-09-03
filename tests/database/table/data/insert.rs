use kasane::grpc::pb;
use kasane_logic::{RangeId, SingleId};

use crate::common::TestApp;
use crate::common::builders::{self, num, text};
use crate::common::data::{assert_first_entry, put_data, search_data, to_result_map};

async fn insert_raw(
    test_app: &TestApp,
    table_name: &str,
    value: prost_types::Value,
    spatial_ids: Vec<pb::SpatialId>,
) -> Result<(), tonic::Status> {
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
        .map(|_| ())
}

fn null_value() -> prost_types::Value {
    prost_types::Value {
        kind: Some(prost_types::value::Kind::NullValue(0)),
    }
}

async fn create_table_with_constraints(
    test_app: &TestApp,
    name: &str,
    data_type: pb::TableDataType,
    max_zoom_level: u32,
    constraints: Option<pb::TableConstraints>,
) {
    test_app
        .table()
        .create(pb::CreateTableRequest {
            db_name: "test_db".to_string(),
            name: name.to_string(),
            data_type: data_type as i32,
            max_zoom_level,
            constraints,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .expect("create_table failed");
}

/// singleIdで指定した空間IDにデータを挿入し、同じ場所から正しく取得できるかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query = vec![builders::single_id(20, 0, 931386, 412905)];

    put_data(&test_app, "test_table", num(3.0), single_id_query.clone()).await;

    let result = search_data(&test_app, "test_table", single_id_query).await;

    assert_first_entry(
        &result,
        &num(3.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );
}

/// Int型の範囲制約（min/max）が挿入時に検証されるかを確認する。
#[tokio::test]
async fn test_table_data_insert_int_range_constraint() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    create_table_with_constraints(
        &test_app,
        "test_table",
        pb::TableDataType::Int,
        25,
        Some(pb::TableConstraints {
            kind: Some(pb::table_constraints::Kind::Int(
                pb::table_constraints::Int {
                    min: Some(-128),
                    max: Some(127),
                },
            )),
        }),
    )
    .await;

    let single_id_query = vec![builders::single_id(20, 0, 931386, 412905)];

    // Valid value
    put_data(&test_app, "test_table", num(127.0), single_id_query.clone()).await;

    let result = search_data(&test_app, "test_table", single_id_query.clone()).await;
    assert_first_entry(
        &result,
        &num(127.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );

    // Invalid value (Out of range)
    let err = insert_raw(&test_app, "test_table", num(128.0), single_id_query)
        .await
        .expect_err("out-of-range value must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

/// singleIdで指定した空間IDに、テーブルの型と一致しない値を挿入した際にエラーが返るかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id_error() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query = vec![builders::single_id(20, 0, 931386, 412905)];

    let err = insert_raw(&test_app, "test_table", text("SampleText"), single_id_query)
        .await
        .expect_err("type mismatch must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

/// 不正なsingleIdを入力した際にエラーが返るかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id_logic_error() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Text", 25)
        .await;

    let single_id_query = vec![builders::single_id(3, 0, 931386, 412905)];

    let err = insert_raw(&test_app, "test_table", text("SampleText"), single_id_query)
        .await
        .expect_err("invalid spatial id must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

/// 2つのsingleIdに対してそれぞれデータが正しく挿入できるかを検証する。
#[tokio::test]
async fn test_table_data_insert_two_single_id() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query_1 = vec![builders::single_id(20, 0, 931386, 412905)];
    put_data(&test_app, "test_table", num(3.0), single_id_query_1.clone()).await;

    let single_id_query_2 = vec![builders::single_id(20, -1, 931386, 412905)];
    put_data(&test_app, "test_table", num(4.0), single_id_query_2.clone()).await;

    let result_1 = search_data(&test_app, "test_table", single_id_query_1).await;
    let result_2 = search_data(&test_app, "test_table", single_id_query_2).await;

    assert_first_entry(
        &result_1,
        &num(3.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );

    assert_first_entry(
        &result_2,
        &num(4.0),
        &pb::SingleId {
            z: 20,
            f: -1,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );
}

/// 同じsingleIdに対してデータを挿入した場合、値が正しく上書きされるかを検証する。
#[tokio::test]
async fn test_table_data_insert_single_id_overwrite() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query = vec![builders::single_id(20, 0, 931386, 412905)];

    put_data(&test_app, "test_table", num(3.0), single_id_query.clone()).await;

    let result = search_data(&test_app, "test_table", single_id_query.clone()).await;

    assert_first_entry(
        &result,
        &num(3.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );

    put_data(&test_app, "test_table", num(4.0), single_id_query.clone()).await;

    let result = search_data(&test_app, "test_table", single_id_query).await;

    assert_first_entry(
        &result,
        &num(4.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );
}

/// 同じrangeIdに対してデータを挿入した場合、値が正しく上書きされるかを検証する。
#[tokio::test]
async fn test_table_data_insert_range_id_overwrite() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table_text", "Text", 25)
        .await;

    let range_id_query = vec![builders::range_id(
        18,
        Some((0, 0)),
        Some((232846, 232850)),
        Some((103226, 103240)),
    )];

    put_data(
        &test_app,
        "test_table_text",
        text("猫(Cat)"),
        range_id_query.clone(),
    )
    .await;

    let result = search_data(&test_app, "test_table_text", range_id_query.clone()).await;
    let result_map = to_result_map::<String>(&result);

    let mut result: Vec<SingleId> = result_map
        .iter()
        .flat_map(|(&(z, f, x, y), value)| {
            assert_eq!(value, "猫(Cat)");
            SingleId::new(z as u8, f, x, y)
                .unwrap()
                .spatial_children_at_zoom(18)
                .unwrap()
                .collect::<Vec<_>>()
        })
        .collect();
    let binding = RangeId::new(18, [0, 0], [232846, 232850], [103226, 103240]).unwrap();
    let mut answer: Vec<SingleId> = binding.single_ids().collect();

    answer.sort();
    result.sort();

    assert_eq!(answer, result);

    put_data(
        &test_app,
        "test_table_text",
        text("犬(Dog)"),
        range_id_query.clone(),
    )
    .await;

    let result = search_data(&test_app, "test_table_text", range_id_query).await;
    let result_map = to_result_map::<String>(&result);

    let mut result: Vec<SingleId> = result_map
        .iter()
        .flat_map(|(&(z, f, x, y), value)| {
            assert_eq!(value, "犬(Dog)");
            SingleId::new(z as u8, f, x, y)
                .unwrap()
                .spatial_children_at_zoom(18)
                .unwrap()
                .collect::<Vec<_>>()
        })
        .collect();
    let binding = RangeId::new(18, [0, 0], [232846, 232850], [103226, 103240]).unwrap();
    let mut answer: Vec<SingleId> = binding.single_ids().collect();

    answer.sort();
    result.sort();

    assert_eq!(answer, result);
}

/// rangeIdで指定した範囲にデータを挿入し、一部・全体それぞれが正しく取得できるかを検証する。
#[tokio::test]
async fn test_table_data_insert_range_id() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let range_id_query = vec![builders::range_id(
        20,
        Some((0, 100)),
        Some((931380, 931386)),
        Some((412900, 412905)),
    )];

    put_data(&test_app, "test_table", num(3.0), range_id_query.clone()).await;

    let single_id_query = vec![builders::single_id(20, 0, 931386, 412905)];
    let result = search_data(&test_app, "test_table", single_id_query).await;

    assert_first_entry(
        &result,
        &num(3.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: None,
            t: None,
        },
    );

    let result = search_data(&test_app, "test_table", range_id_query).await;
    let result_map = to_result_map::<i64>(&result);

    assert_eq!(result_map.len(), 917);

    let mut answer: Vec<SingleId> = RangeId::new(20, [0, 100], [931380, 931386], [412900, 412905])
        .unwrap()
        .single_ids()
        .collect();

    let mut result: Vec<SingleId> = result_map
        .iter()
        .flat_map(|(&(z, f, x, y), &value)| {
            assert_eq!(value, 3);
            SingleId::new(z as u8, f, x, y)
                .unwrap()
                .spatial_children_at_zoom(20)
                .unwrap()
                .collect::<Vec<_>>()
        })
        .collect();

    answer.sort();
    result.sort();
    assert_eq!(answer, result);
}

/// Insertを用いて一部の値の上書きを行った際、新しい値と元の値が正しい状態を保つかを検証する。
#[tokio::test]
async fn test_table_data_overload_insert() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Text", 30)
        .await;

    let query1 = vec![builders::single_id(20, 0, 931386, 412905)];
    put_data(&test_app, "test_table", text("A"), query1.clone()).await;

    let query2 = vec![builders::single_id(21, 0, 1862772, 825810)];
    put_data(&test_app, "test_table", text("B"), query2).await;

    let result = search_data(&test_app, "test_table", query1).await;
    let result_map = to_result_map::<String>(&result);

    assert_eq!(result_map.len(), 8);

    let overload_single_id = (21u32, 0i32, 1862772u32, 825810u32);

    for (raw_single_id, value) in result_map {
        if raw_single_id == overload_single_id {
            assert_eq!(value, "B".to_string());
        } else {
            assert_eq!(value, "A".to_string());
        }
    }
}

#[tokio::test]
/// 64個のノード（Zoom 20）を順次挿入した際、再帰的にマージされて1つのZoom 18ノードになるかを検証する。
async fn test_table_data_recursive_merge() {
    let test_app = TestApp::new().await;

    let table_name = "merge_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    for f in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                let single_id_query = vec![builders::single_id(20, f, x, y)];
                put_data(&test_app, table_name, num(7.0), single_id_query).await;
            }
        }
    }

    let search_query = vec![builders::single_id(18, 0, 0, 0)];
    let result = search_data(&test_app, table_name, search_query).await;
    let result_map = to_result_map::<i64>(&result);

    assert_eq!(
        result_map.len(),
        1,
        "Should be merged into a single node, but found: {:?}",
        result_map
    );

    let (&(z, f, x, y), &value) = result_map.iter().next().unwrap();
    assert_eq!(z, 18);
    assert_eq!(f, 0);
    assert_eq!(x, 0);
    assert_eq!(y, 0);
    assert_eq!(value, 7);
}

#[tokio::test]
/// 異なるテーブル間で同じ座標にデータを挿入しても、互いに干渉しないかを検証する。
async fn test_table_data_isolation() {
    let test_app = TestApp::new().await;

    let table1 = "table1";
    let table2 = "table2";

    test_app.create_database("test_db").await;
    test_app.create_table("test_db", table1, "Int", 25).await;
    test_app.create_table("test_db", table2, "Int", 25).await;

    let query = vec![builders::single_id(20, 0, 100, 100)];

    put_data(&test_app, table1, num(1.0), query.clone()).await;
    put_data(&test_app, table2, num(2.0), query.clone()).await;

    let res1 = search_data(&test_app, table1, query.clone()).await;
    assert_first_entry(
        &res1,
        &num(1.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 100,
            y: 100,
            i: None,
            t: None,
        },
    );

    let res2 = search_data(&test_app, table2, query).await;
    assert_first_entry(
        &res2,
        &num(2.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 100,
            y: 100,
            i: None,
            t: None,
        },
    );
}

#[tokio::test]
/// max_zoom_levelを超えるズームレベルでの挿入がエラーになるかを検証する。
async fn test_table_data_max_zoom_enforcement() {
    let test_app = TestApp::new().await;

    let table_name = "low_zoom_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 10)
        .await;

    let high_zoom_query = vec![builders::single_id(11, 0, 0, 0)];

    let err = insert_raw(&test_app, table_name, num(100.0), high_zoom_query)
        .await
        .expect_err("zoom level beyond max_zoom_level must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
/// 広範な親ノード内にピンポイントな子ノードを挿入した際、親が適切に分割され値の整合性が保たれるかを検証する。
async fn test_table_data_deep_split() {
    let test_app = TestApp::new().await;

    let table_name = "split_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let parent_query = vec![builders::single_id(18, 0, 0, 0)];
    put_data(&test_app, table_name, num(100.0), parent_query).await;

    let child_query = vec![builders::single_id(20, 0, 0, 0)];
    put_data(&test_app, table_name, num(200.0), child_query.clone()).await;

    let res_child = search_data(&test_app, table_name, child_query).await;
    assert_first_entry(
        &res_child,
        &num(200.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 0,
            y: 0,
            i: None,
            t: None,
        },
    );

    let sibling_query = vec![builders::single_id(20, 0, 1, 0)];
    let res_sibling = search_data(&test_app, table_name, sibling_query).await;
    assert_first_entry(
        &res_sibling,
        &num(100.0),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 1,
            y: 0,
            i: None,
            t: None,
        },
    );
}

#[tokio::test]
/// Enum型のテーブルに対して、許可された値の挿入が成功することを検証する。
async fn test_table_data_insert_enum_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    create_table_with_constraints(
        &test_app,
        "enum_table",
        pb::TableDataType::Enum,
        25,
        Some(pb::TableConstraints {
            kind: Some(pb::table_constraints::Kind::EnumConstraint(
                pb::table_constraints::Enum {
                    choices: vec![
                        "Apple".to_string(),
                        "Banana".to_string(),
                        "Orange".to_string(),
                    ],
                },
            )),
        }),
    )
    .await;

    let single_id_query = vec![builders::single_id(20, 0, 0, 0)];

    put_data(
        &test_app,
        "enum_table",
        text("Banana"),
        single_id_query.clone(),
    )
    .await;

    let result = search_data(&test_app, "enum_table", single_id_query).await;
    assert_first_entry(
        &result,
        &text("Banana"),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 0,
            y: 0,
            i: None,
            t: None,
        },
    );
}

#[tokio::test]
/// Enum型のテーブルに対して、許可されていない値の挿入が失敗することを検証する。
async fn test_table_data_insert_enum_failure() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    create_table_with_constraints(
        &test_app,
        "enum_table",
        pb::TableDataType::Enum,
        25,
        Some(pb::TableConstraints {
            kind: Some(pb::table_constraints::Kind::EnumConstraint(
                pb::table_constraints::Enum {
                    choices: vec![
                        "Apple".to_string(),
                        "Banana".to_string(),
                        "Orange".to_string(),
                    ],
                },
            )),
        }),
    )
    .await;

    let single_id_query = vec![builders::single_id(20, 0, 0, 0)];

    let err = insert_raw(&test_app, "enum_table", text("Grape"), single_id_query)
        .await
        .expect_err("value outside the enum choices must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
/// Presence型のテーブルに対して、null の挿入が成功することを検証する。
async fn test_table_data_insert_presence_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    create_table_with_constraints(
        &test_app,
        "presence_table",
        pb::TableDataType::Presence,
        25,
        None,
    )
    .await;

    let single_id_query = vec![builders::single_id(20, 0, 0, 0)];

    put_data(
        &test_app,
        "presence_table",
        null_value(),
        single_id_query.clone(),
    )
    .await;

    let result = search_data(&test_app, "presence_table", single_id_query).await;

    assert_first_entry(
        &result,
        &null_value(),
        &pb::SingleId {
            z: 20,
            f: 0,
            x: 0,
            y: 0,
            i: None,
            t: None,
        },
    );
}

#[tokio::test]
/// Presence型のテーブルに対して、null 以外の値の挿入が失敗することを検証する。
async fn test_table_data_insert_presence_failure() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    create_table_with_constraints(
        &test_app,
        "presence_table",
        pb::TableDataType::Presence,
        25,
        None,
    )
    .await;

    let single_id_query = vec![builders::single_id(20, 0, 0, 0)];

    let err = insert_raw(
        &test_app,
        "presence_table",
        text("some_value"),
        single_id_query,
    )
    .await
    .expect_err("non-null value into a Presence table must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
/// 格納バイト数の上限（`MAX_STORED_VALUE_BYTES`）を超える値の挿入が拒否されることを検証する。
///
/// 1 つの値は挿入対象の空間 ID ごとに複製されて葉へ書かれるので、上限が無いと 1 回の挿入
/// だけでシャードのバイト数上限を超えうる（詳細は
/// `kasane::services::helpers::value::MAX_STORED_VALUE_BYTES` のコメントを参照）。
async fn test_table_data_insert_value_exceeds_max_size() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "text_table", "Text", 25)
        .await;

    let single_id_query = vec![builders::single_id(20, 0, 0, 0)];

    // MAX_STORED_VALUE_BYTES (256 KiB) を超える文字列。
    let oversized_value = "a".repeat(257 * 1024);

    let err = insert_raw(
        &test_app,
        "text_table",
        text(&oversized_value),
        single_id_query,
    )
    .await
    .expect_err("value exceeding MAX_STORED_VALUE_BYTES must be rejected");
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}
