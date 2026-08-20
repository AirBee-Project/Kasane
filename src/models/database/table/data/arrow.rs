use crate::models::database::table::data::{
    GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
};
use arrow::array::{
    ArrayRef, DictionaryArray, Float64Builder, Int32Builder, StringBuilder, UInt8Array,
    UInt8Builder, UInt32Array, UInt32Builder, UInt64Builder,
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

fn build_batch_single(
    r: GetDataResponseSingle,
) -> Result<(Vec<Value>, RecordBatch), arrow::error::ArrowError> {
    let mut value_refs = Vec::new();
    let mut zs = Vec::new();
    let mut fs = Vec::new();
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut is = UInt64Builder::new();
    let mut ts = UInt64Builder::new();

    for group in r.data {
        for sid in group.spatial_ids {
            value_refs.push(group.value_ref as u32);
            zs.push(sid.z);
            fs.push(sid.f);
            xs.push(sid.x);
            ys.push(sid.y);

            if let Some(i) = sid.i {
                is.append_value(i);
            } else {
                is.append_null();
            }
            if let Some(t) = sid.t {
                ts.append_value(t);
            } else {
                ts.append_null();
            }
        }
    }

    let value_col = build_dictionary_array(&r.dictionary, value_refs)?;
    let value_type = value_col.data_type().clone();

    let schema = Arc::new(Schema::new(vec![
        Field::new("value", value_type, true),
        Field::new("z", DataType::UInt8, false),
        Field::new("f", DataType::Int32, false),
        Field::new("x", DataType::UInt32, false),
        Field::new("y", DataType::UInt32, false),
        Field::new("i", DataType::UInt64, true),
        Field::new("t", DataType::UInt64, true),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            value_col,
            Arc::new(UInt8Array::from(zs)),
            Arc::new(arrow::array::Int32Array::from(fs)),
            Arc::new(arrow::array::UInt32Array::from(xs)),
            Arc::new(arrow::array::UInt32Array::from(ys)),
            Arc::new(is.finish()),
            Arc::new(ts.finish()),
        ],
    )?;

    Ok((r.dictionary, batch))
}

fn build_batch_range(
    r: GetDataResponseRange,
) -> Result<(Vec<Value>, RecordBatch), arrow::error::ArrowError> {
    let mut value_refs = Vec::new();
    let mut zs = Vec::new();
    let mut f_mins = Int32Builder::new();
    let mut f_maxs = Int32Builder::new();
    let mut x_mins = UInt32Builder::new();
    let mut x_maxs = UInt32Builder::new();
    let mut y_mins = UInt32Builder::new();
    let mut y_maxs = UInt32Builder::new();
    let mut is = UInt64Builder::new();
    let mut t_mins = UInt64Builder::new();
    let mut t_maxs = UInt64Builder::new();

    for group in r.data {
        for sid in group.spatial_ids {
            value_refs.push(group.value_ref as u32);
            zs.push(sid.z);

            if let Some(f) = sid.f {
                f_mins.append_value(f[0]);
                f_maxs.append_value(f[1]);
            } else {
                f_mins.append_null();
                f_maxs.append_null();
            }

            if let Some(x) = sid.x {
                x_mins.append_value(x[0]);
                x_maxs.append_value(x[1]);
            } else {
                x_mins.append_null();
                x_maxs.append_null();
            }

            if let Some(y) = sid.y {
                y_mins.append_value(y[0]);
                y_maxs.append_value(y[1]);
            } else {
                y_mins.append_null();
                y_maxs.append_null();
            }

            if let Some(i) = sid.i {
                is.append_value(i);
            } else {
                is.append_null();
            }

            if let Some(t) = sid.t {
                t_mins.append_value(t[0]);
                t_maxs.append_value(t[1]);
            } else {
                t_mins.append_null();
                t_maxs.append_null();
            }
        }
    }

    let value_col = build_dictionary_array(&r.dictionary, value_refs)?;
    let value_type = value_col.data_type().clone();

    let schema = Arc::new(Schema::new(vec![
        Field::new("value", value_type, true),
        Field::new("z", DataType::UInt8, false),
        Field::new("fMin", DataType::Int32, true),
        Field::new("fMax", DataType::Int32, true),
        Field::new("xMin", DataType::UInt32, true),
        Field::new("xMax", DataType::UInt32, true),
        Field::new("yMin", DataType::UInt32, true),
        Field::new("yMax", DataType::UInt32, true),
        Field::new("i", DataType::UInt64, true),
        Field::new("tMin", DataType::UInt64, true),
        Field::new("tMax", DataType::UInt64, true),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            value_col,
            Arc::new(UInt8Array::from(zs)),
            Arc::new(f_mins.finish()),
            Arc::new(f_maxs.finish()),
            Arc::new(x_mins.finish()),
            Arc::new(x_maxs.finish()),
            Arc::new(y_mins.finish()),
            Arc::new(y_maxs.finish()),
            Arc::new(is.finish()),
            Arc::new(t_mins.finish()),
            Arc::new(t_maxs.finish()),
        ],
    )?;

    Ok((r.dictionary, batch))
}

fn build_batch_flex(
    r: GetDataResponseFlex,
) -> Result<(Vec<Value>, RecordBatch), arrow::error::ArrowError> {
    let mut value_refs = Vec::new();
    let mut f_zs = Vec::new();
    let mut f_is = Vec::new();
    let mut x_zs = Vec::new();
    let mut x_is = Vec::new();
    let mut y_zs = Vec::new();
    let mut y_is = Vec::new();
    let mut t_zs = UInt8Builder::new();
    let mut t_is = UInt64Builder::new();

    for group in r.data {
        for sid in group.spatial_ids {
            value_refs.push(group.value_ref as u32);
            f_zs.push(sid.f_zoomlevel);
            f_is.push(sid.f_index);
            x_zs.push(sid.x_zoomlevel);
            x_is.push(sid.x_index);
            y_zs.push(sid.y_zoomlevel);
            y_is.push(sid.y_index);

            if let Some(z) = sid.t_zoomlevel {
                t_zs.append_value(z);
            } else {
                t_zs.append_null();
            }

            if let Some(i) = sid.t_index {
                t_is.append_value(i);
            } else {
                t_is.append_null();
            }
        }
    }

    let value_col = build_dictionary_array(&r.dictionary, value_refs)?;
    let value_type = value_col.data_type().clone();

    let schema = Arc::new(Schema::new(vec![
        Field::new("value", value_type, true),
        Field::new("fZoomlevel", DataType::UInt8, false),
        Field::new("fIndex", DataType::Int32, false),
        Field::new("xZoomlevel", DataType::UInt8, false),
        Field::new("xIndex", DataType::UInt32, false),
        Field::new("yZoomlevel", DataType::UInt8, false),
        Field::new("yIndex", DataType::UInt32, false),
        Field::new("tZoomlevel", DataType::UInt8, true),
        Field::new("tIndex", DataType::UInt64, true),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![
            value_col,
            Arc::new(UInt8Array::from(f_zs)),
            Arc::new(arrow::array::Int32Array::from(f_is)),
            Arc::new(UInt8Array::from(x_zs)),
            Arc::new(arrow::array::UInt32Array::from(x_is)),
            Arc::new(UInt8Array::from(y_zs)),
            Arc::new(arrow::array::UInt32Array::from(y_is)),
            Arc::new(t_zs.finish()),
            Arc::new(t_is.finish()),
        ],
    )?;

    Ok((r.dictionary, batch))
}
