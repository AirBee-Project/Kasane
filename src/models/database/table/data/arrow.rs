use crate::models::database::table::data::{
    GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
};
use arrow::array::{
    ArrayRef, DictionaryArray, Float64Builder, Int32Array, StringBuilder, UInt8Array, UInt32Array,
};
use arrow::datatypes::{DataType, Field, Schema, UInt32Type};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use serde_json::Value;
use std::sync::Arc;

pub fn to_arrow_ipc(response: GetDataResponse) -> Result<Vec<u8>, arrow::error::ArrowError> {
    let (_dictionary, batch) = match response {
        GetDataResponse::Single(r) => build_batch_single(r)?,
        GetDataResponse::Range(r) => build_batch_range(r)?,
        GetDataResponse::Flex(r) => build_batch_flex(r)?,
    };

    let mut buf = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut buf, &batch.schema())?;
        writer.write(&batch)?;
        writer.finish()?;
    }
    Ok(buf)
}

fn build_dictionary_array(
    dictionary: &[Value],
    value_refs: Vec<u32>,
) -> Result<ArrayRef, arrow::error::ArrowError> {
    let is_numeric = dictionary.iter().all(|v| v.is_number() || v.is_null());

    let dict_values: ArrayRef = if is_numeric {
        let mut builder = Float64Builder::with_capacity(dictionary.len());
        for v in dictionary {
            if let Some(n) = v.as_f64() {
                builder.append_value(n);
            } else {
                builder.append_null();
            }
        }
        Arc::new(builder.finish())
    } else {
        let mut builder = StringBuilder::with_capacity(dictionary.len(), dictionary.len() * 10);
        for v in dictionary {
            if v.is_null() {
                builder.append_null();
            } else if let Some(s) = v.as_str() {
                builder.append_value(s);
            } else {
                builder.append_value(v.to_string());
            }
        }
        Arc::new(builder.finish())
    };

    let keys = UInt32Array::from(value_refs);
    let dict_array = DictionaryArray::<UInt32Type>::try_new(keys, dict_values)?;
    Ok(Arc::new(dict_array))
}

fn build_batch_range(
    r: GetDataResponseRange,
) -> Result<(Vec<Value>, RecordBatch), arrow::error::ArrowError> {
    let mut value_refs = Vec::new();
    let mut zs = Vec::new();
    let mut f_mins = Vec::new();
    let mut f_maxs = Vec::new();
    let mut x_mins = Vec::new();
    let mut x_maxs = Vec::new();
    let mut y_mins = Vec::new();
    let mut y_maxs = Vec::new();

    for group in r.data {
        for sid in group.spatial_ids {
            value_refs.push(group.value_ref as u32);
            zs.push(sid.z);

            if let Some(f) = sid.f {
                f_mins.push(f[0]);
                f_maxs.push(f[1]);
            } else {
                f_mins.push(0);
                f_maxs.push(0);
            }

            if let Some(x) = sid.x {
                x_mins.push(x[0]);
                x_maxs.push(x[1]);
            } else {
                x_mins.push(0);
                x_maxs.push(0);
            }

            if let Some(y) = sid.y {
                y_mins.push(y[0]);
                y_maxs.push(y[1]);
            } else {
                y_mins.push(0);
                y_maxs.push(0);
            }
        }
    }

    let value_col = build_dictionary_array(&r.dictionary, value_refs)?;
    let value_type = value_col.data_type().clone();

    let schema = Arc::new(Schema::new(vec![
        Field::new("value", value_type, true),
        Field::new("z", DataType::UInt8, false),
        Field::new("fMin", DataType::Int32, false),
        Field::new("fMax", DataType::Int32, false),
        Field::new("xMin", DataType::UInt32, false),
        Field::new("xMax", DataType::UInt32, false),
        Field::new("yMin", DataType::UInt32, false),
        Field::new("yMax", DataType::UInt32, false),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            value_col,
            Arc::new(UInt8Array::from(zs)),
            Arc::new(Int32Array::from(f_mins)),
            Arc::new(Int32Array::from(f_maxs)),
            Arc::new(UInt32Array::from(x_mins)),
            Arc::new(UInt32Array::from(x_maxs)),
            Arc::new(UInt32Array::from(y_mins)),
            Arc::new(UInt32Array::from(y_maxs)),
        ],
    )?;

    Ok((r.dictionary, batch))
}

fn build_batch_single(
    r: GetDataResponseSingle,
) -> Result<(Vec<Value>, RecordBatch), arrow::error::ArrowError> {
    let mut value_refs = Vec::new();
    let mut zs = Vec::new();
    let mut f_mins = Vec::new();
    let mut f_maxs = Vec::new();
    let mut x_mins = Vec::new();
    let mut x_maxs = Vec::new();
    let mut y_mins = Vec::new();
    let mut y_maxs = Vec::new();

    for group in r.data {
        for sid in group.spatial_ids {
            value_refs.push(group.value_ref as u32);
            zs.push(sid.z);
            f_mins.push(sid.f);
            f_maxs.push(sid.f);
            x_mins.push(sid.x);
            x_maxs.push(sid.x);
            y_mins.push(sid.y);
            y_maxs.push(sid.y);
        }
    }

    let value_col = build_dictionary_array(&r.dictionary, value_refs)?;
    let value_type = value_col.data_type().clone();

    let schema = Arc::new(Schema::new(vec![
        Field::new("value", value_type, true),
        Field::new("z", DataType::UInt8, false),
        Field::new("fMin", DataType::Int32, false),
        Field::new("fMax", DataType::Int32, false),
        Field::new("xMin", DataType::UInt32, false),
        Field::new("xMax", DataType::UInt32, false),
        Field::new("yMin", DataType::UInt32, false),
        Field::new("yMax", DataType::UInt32, false),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            value_col,
            Arc::new(UInt8Array::from(zs)),
            Arc::new(Int32Array::from(f_mins)),
            Arc::new(Int32Array::from(f_maxs)),
            Arc::new(UInt32Array::from(x_mins)),
            Arc::new(UInt32Array::from(x_maxs)),
            Arc::new(UInt32Array::from(y_mins)),
            Arc::new(UInt32Array::from(y_maxs)),
        ],
    )?;

    Ok((r.dictionary, batch))
}

fn build_batch_flex(
    r: GetDataResponseFlex,
) -> Result<(Vec<Value>, RecordBatch), arrow::error::ArrowError> {
    let mut value_refs = Vec::new();
    let mut zs = Vec::new();
    let mut f_mins = Vec::new();
    let mut f_maxs = Vec::new();
    let mut x_mins = Vec::new();
    let mut x_maxs = Vec::new();
    let mut y_mins = Vec::new();
    let mut y_maxs = Vec::new();

    for group in r.data {
        for sid in group.spatial_ids {
            value_refs.push(group.value_ref as u32);
            // FlexId requires more complex mapping, but to match the schema we fallback to x_zoomlevel
            zs.push(sid.x_zoomlevel);
            f_mins.push(sid.f_index);
            f_maxs.push(sid.f_index);
            x_mins.push(sid.x_index);
            x_maxs.push(sid.x_index);
            y_mins.push(sid.y_index);
            y_maxs.push(sid.y_index);
        }
    }

    let value_col = build_dictionary_array(&r.dictionary, value_refs)?;
    let value_type = value_col.data_type().clone();

    let schema = Arc::new(Schema::new(vec![
        Field::new("value", value_type, true),
        Field::new("z", DataType::UInt8, false),
        Field::new("fMin", DataType::Int32, false),
        Field::new("fMax", DataType::Int32, false),
        Field::new("xMin", DataType::UInt32, false),
        Field::new("xMax", DataType::UInt32, false),
        Field::new("yMin", DataType::UInt32, false),
        Field::new("yMax", DataType::UInt32, false),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            value_col,
            Arc::new(UInt8Array::from(zs)),
            Arc::new(Int32Array::from(f_mins)),
            Arc::new(Int32Array::from(f_maxs)),
            Arc::new(UInt32Array::from(x_mins)),
            Arc::new(UInt32Array::from(x_maxs)),
            Arc::new(UInt32Array::from(y_mins)),
            Arc::new(UInt32Array::from(y_maxs)),
        ],
    )?;

    Ok((r.dictionary, batch))
}
