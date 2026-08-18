use arrow::array::{
    Array, DictionaryArray, Int32Array, StringArray, UInt32Array, UInt64Array, UInt8Array,
};
use arrow::ipc::reader::StreamReader;
use kasane::models::database::table::data::{
    DataGroup, GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
};
use kasane::models::spatial_id::{RawFlexId, RawRangeId, RawSingleId};
use serde_json::json;

#[test]
fn test_arrow_export_single() {
    let r = GetDataResponse::Single(GetDataResponseSingle {
        dictionary: vec![json!("value1"), json!("value2")],
        data: vec![DataGroup {
            value_ref: 1,
            spatial_ids: vec![RawSingleId {
                z: 20,
                f: 0,
                x: 931386,
                y: 412905,
                i: Some(3600),
                t: None,
            }],
        }],
    });

    let bytes = kasane::models::database::table::data::arrow::to_arrow_ipc(r).unwrap();
    let mut reader = StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
    let batch = reader.next().unwrap().unwrap();

    assert_eq!(batch.num_columns(), 7);
    assert_eq!(batch.num_rows(), 1);

    let z_col = batch
        .column(1)
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(z_col.value(0), 20);

    let i_col = batch
        .column(5)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(i_col.value(0), 3600);
    assert!(i_col.is_valid(0));

    let t_col = batch
        .column(6)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert!(t_col.is_null(0));
}

#[test]
fn test_arrow_export_range_with_missing_axes() {
    let r = GetDataResponse::Range(GetDataResponseRange {
        dictionary: vec![json!(10.5), json!(20.0)],
        data: vec![DataGroup {
            value_ref: 0,
            spatial_ids: vec![RawRangeId {
                z: 15,
                f: None, // 全範囲
                x: Some([10, 20]),
                y: None, // 全範囲
                i: None,
                t: None,
            }],
        }],
    });

    let bytes = kasane::models::database::table::data::arrow::to_arrow_ipc(r).unwrap();
    let mut reader = StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
    let batch = reader.next().unwrap().unwrap();

    let f_min = batch
        .column(2)
        .as_any()
        .downcast_ref::<Int32Array>()
        .unwrap();
    assert!(f_min.is_null(0), "fMin should be null for omitted f axis");

    let x_max = batch
        .column(5)
        .as_any()
        .downcast_ref::<UInt32Array>()
        .unwrap();
    assert_eq!(x_max.value(0), 20);
    assert!(x_max.is_valid(0));

    let y_min = batch
        .column(6)
        .as_any()
        .downcast_ref::<UInt32Array>()
        .unwrap();
    assert!(y_min.is_null(0), "yMin should be null for omitted y axis");
}

#[test]
fn test_arrow_export_flex() {
    let r = GetDataResponse::Flex(GetDataResponseFlex {
        dictionary: vec![json!("a")],
        data: vec![DataGroup {
            value_ref: 0,
            spatial_ids: vec![RawFlexId {
                f_zoomlevel: 10,
                f_index: 0,
                x_zoomlevel: 20,
                x_index: 100,
                y_zoomlevel: 21,
                y_index: 200,
                t_zoomlevel: Some(30),
                t_index: Some(1000),
            }],
        }],
    });

    let bytes = kasane::models::database::table::data::arrow::to_arrow_ipc(r).unwrap();
    let mut reader = StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
    let batch = reader.next().unwrap().unwrap();

    // Field::new("fZoomlevel", DataType::UInt8, false), (idx 1)
    let f_z = batch
        .column(1)
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(f_z.value(0), 10);

    // xZoomlevel (idx 3)
    let x_z = batch
        .column(3)
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(x_z.value(0), 20);

    // yZoomlevel (idx 5)
    let y_z = batch
        .column(5)
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(y_z.value(0), 21);

    // tZoomlevel (idx 7)
    let t_z = batch
        .column(7)
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(t_z.value(0), 30);
    assert!(t_z.is_valid(0));
}
