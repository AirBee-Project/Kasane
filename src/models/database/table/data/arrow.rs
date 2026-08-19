use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use kasane_logic::{AllowedIntervals, FlexId, SpatialId as _, SpatialIdSet};
use serde_json::Value;

use crate::{
    error::AppError,
    models::database::table::data::{
        GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
        OutputFormat,
    },
};
use arrow::array::{
    DictionaryArray, Float64Builder, Int32Builder, StringBuilder, UInt8Array, UInt8Builder,
    UInt32Array, UInt32Builder, UInt64Builder,
};
use arrow::datatypes::{DataType, Field, Schema, UInt32Type};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
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
) -> Result<Arc<dyn arrow::array::Array>, arrow::error::ArrowError> {
    let all_numbers = dictionary.iter().all(|v| v.is_null() || v.is_number());
    let all_bools = dictionary.iter().all(|v| v.is_null() || v.is_boolean());

    let dict_values: Arc<dyn arrow::array::Array> = if all_numbers {
        let mut builder = Float64Builder::with_capacity(dictionary.len());
        for v in dictionary {
            if v.is_null() {
                builder.append_null();
            } else if let Some(n) = v.as_f64() {
                builder.append_value(n);
            } else {
                builder.append_null();
            }
        }
        Arc::new(builder.finish())
    } else if all_bools {
        let mut builder = arrow::array::BooleanBuilder::with_capacity(dictionary.len());
        for v in dictionary {
            if v.is_null() {
                builder.append_null();
            } else if let Some(b) = v.as_bool() {
                builder.append_value(b);
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

fn take_one(limit_left: &mut Option<usize>) -> bool {
    match limit_left {
        Some(0) => false,
        Some(left) => {
            *left -= 1;
            true
        }
        None => true,
    }
}

/// 1 バッチに積む行数。大きすぎるとバッチ生成中のピークメモリが増え、小さすぎると
/// IPC メッセージのオーバーヘッドが相対的に増える。
const ARROW_BATCH_ROWS: usize = 4096;

struct ChannelWriter {
    tx: tokio::sync::mpsc::Sender<Result<Bytes, String>>,
}
impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self
            .tx
            .blocking_send(Ok(Bytes::copy_from_slice(buf)))
            .is_err()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "channel closed",
            ));
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// 失敗を握りつぶさず、書き手側にエラーとして伝える。すでに 200 応答が始まっているので
/// ステータスコードは変えられないが、ストリームを異常終了させることで無言の欠損を防ぐ。
fn fail_stream(tx: &tokio::sync::mpsc::Sender<Result<Bytes, String>>, msg: impl Into<String>) {
    let _ = tx.blocking_send(Err(msg.into()));
}

macro_rules! try_arrow {
    ($tx:expr, $expr:expr, $ctx:literal) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                fail_stream($tx, format!("{}: {e}", $ctx));
                return;
            }
        }
    };
}

pub fn stream_arrow_ipc<V, I, F>(
    groups: I,
    format: OutputFormat,
    limit: Option<usize>,
    to_json: F,
) -> Result<Response, AppError>
where
    I: IntoIterator<Item = (V, Vec<FlexId>)>,
    F: Fn(&V) -> Result<Value, AppError>,
{
    let mut dictionary: Vec<Value> = Vec::new();
    let mut limit_left = limit;

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, String>>(4);

    match format {
        OutputFormat::SingleId => {
            let mut value_refs: Vec<u32> = Vec::new();
            let mut zs: Vec<u8> = Vec::new();
            let mut fs: Vec<i32> = Vec::new();
            let mut xs: Vec<u32> = Vec::new();
            let mut ys: Vec<u32> = Vec::new();
            let mut is_vals: Vec<Option<u64>> = Vec::new();
            let mut ts_vals: Vec<Option<u64>> = Vec::new();

            'groups: for (value, flex_ids) in groups {
                if limit_left == Some(0) {
                    break;
                }
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                let start = zs.len();
                'ranges: for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
                    for single_id in range_id.single_ids() {
                        if !take_one(&mut limit_left) {
                            break 'ranges;
                        }
                        zs.push(single_id.z());
                        fs.push(single_id.f());
                        xs.push(single_id.x());
                        ys.push(single_id.y());
                        if single_id.is_whole_time() {
                            is_vals.push(None);
                            ts_vals.push(None);
                        } else {
                            is_vals.push(Some(single_id.time_interval().seconds()));
                            ts_vals.push(Some(single_id.t()));
                        }
                    }
                }
                // 先に push すると、limit で data に載らなかった値が孤立した辞書項目として残る。
                if zs.len() == start {
                    continue 'groups;
                }
                let value_ref = dictionary.len() as u32;
                dictionary.push(to_json(&value)?);
                value_refs.resize(zs.len(), value_ref);
            }

            let schema = Arc::new(Schema::new(vec![
                Field::new(
                    "value",
                    build_dictionary_array(&dictionary, vec![])
                        .map_err(|e| AppError::InternalError(e.to_string()))?
                        .data_type()
                        .clone(),
                    true,
                ),
                Field::new("z", DataType::UInt8, false),
                Field::new("f", DataType::Int32, false),
                Field::new("x", DataType::UInt32, false),
                Field::new("y", DataType::UInt32, false),
                Field::new("i", DataType::UInt64, true),
                Field::new("t", DataType::UInt64, true),
            ]));

            tokio::task::spawn_blocking(move || {
                let mut writer_sink = ChannelWriter { tx: tx.clone() };
                let mut writer = try_arrow!(
                    &tx,
                    StreamWriter::try_new(&mut writer_sink, &schema),
                    "arrow stream init failed"
                );

                let total = zs.len();
                let mut offset = 0;
                while offset < total {
                    let end = (offset + ARROW_BATCH_ROWS).min(total);

                    let value_col = try_arrow!(
                        &tx,
                        build_dictionary_array(&dictionary, value_refs[offset..end].to_vec()),
                        "dictionary array build failed"
                    );

                    let mut is = UInt64Builder::new();
                    let mut ts = UInt64Builder::new();
                    for i in offset..end {
                        match is_vals[i] {
                            Some(v) => is.append_value(v),
                            None => is.append_null(),
                        }
                        match ts_vals[i] {
                            Some(v) => ts.append_value(v),
                            None => ts.append_null(),
                        }
                    }

                    let batch = try_arrow!(
                        &tx,
                        RecordBatch::try_new(
                            schema.clone(),
                            vec![
                                value_col,
                                Arc::new(UInt8Array::from(zs[offset..end].to_vec())),
                                Arc::new(arrow::array::Int32Array::from(fs[offset..end].to_vec())),
                                Arc::new(arrow::array::UInt32Array::from(xs[offset..end].to_vec())),
                                Arc::new(arrow::array::UInt32Array::from(ys[offset..end].to_vec())),
                                Arc::new(is.finish()),
                                Arc::new(ts.finish()),
                            ],
                        ),
                        "record batch build failed"
                    );

                    if writer.write(&batch).is_err() {
                        return;
                    }
                    offset = end;
                }

                if let Err(e) = writer.finish() {
                    fail_stream(&tx, format!("arrow stream finish failed: {e}"));
                }
            });
        }
        OutputFormat::RangeId => {
            let mut value_refs: Vec<u32> = Vec::new();
            let mut zs: Vec<u8> = Vec::new();
            let mut f_mins: Vec<i32> = Vec::new();
            let mut f_maxs: Vec<i32> = Vec::new();
            let mut x_mins: Vec<u32> = Vec::new();
            let mut x_maxs: Vec<u32> = Vec::new();
            let mut y_mins: Vec<u32> = Vec::new();
            let mut y_maxs: Vec<u32> = Vec::new();
            let mut is_vals: Vec<Option<u64>> = Vec::new();
            let mut t_mins: Vec<Option<u64>> = Vec::new();
            let mut t_maxs: Vec<Option<u64>> = Vec::new();

            'groups: for (value, flex_ids) in groups {
                if limit_left == Some(0) {
                    break;
                }
                let set: SpatialIdSet = flex_ids.into_iter().collect();
                let start = zs.len();
                for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
                    if !take_one(&mut limit_left) {
                        break;
                    }
                    zs.push(range_id.z());
                    f_mins.push(range_id.f()[0]);
                    f_maxs.push(range_id.f()[1]);
                    x_mins.push(range_id.x()[0]);
                    x_maxs.push(range_id.x()[1]);
                    y_mins.push(range_id.y()[0]);
                    y_maxs.push(range_id.y()[1]);
                    if range_id.is_whole_time() {
                        is_vals.push(None);
                        t_mins.push(None);
                        t_maxs.push(None);
                    } else {
                        is_vals.push(Some(range_id.time_interval().seconds()));
                        t_mins.push(Some(range_id.t()[0]));
                        t_maxs.push(Some(range_id.t()[1]));
                    }
                }
                // 先に push すると、limit で data に載らなかった値が孤立した辞書項目として残る。
                if zs.len() == start {
                    continue 'groups;
                }
                let value_ref = dictionary.len() as u32;
                dictionary.push(to_json(&value)?);
                value_refs.resize(zs.len(), value_ref);
            }

            let schema = Arc::new(Schema::new(vec![
                Field::new(
                    "value",
                    build_dictionary_array(&dictionary, vec![])
                        .map_err(|e| AppError::InternalError(e.to_string()))?
                        .data_type()
                        .clone(),
                    true,
                ),
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

            tokio::task::spawn_blocking(move || {
                let mut writer_sink = ChannelWriter { tx: tx.clone() };
                let mut writer = try_arrow!(
                    &tx,
                    StreamWriter::try_new(&mut writer_sink, &schema),
                    "arrow stream init failed"
                );

                let total = zs.len();
                let mut offset = 0;
                while offset < total {
                    let end = (offset + ARROW_BATCH_ROWS).min(total);

                    let value_col = try_arrow!(
                        &tx,
                        build_dictionary_array(&dictionary, value_refs[offset..end].to_vec()),
                        "dictionary array build failed"
                    );

                    let mut is = UInt64Builder::new();
                    let mut t_min_b = UInt64Builder::new();
                    let mut t_max_b = UInt64Builder::new();
                    for i in offset..end {
                        match is_vals[i] {
                            Some(v) => is.append_value(v),
                            None => is.append_null(),
                        }
                        match t_mins[i] {
                            Some(v) => t_min_b.append_value(v),
                            None => t_min_b.append_null(),
                        }
                        match t_maxs[i] {
                            Some(v) => t_max_b.append_value(v),
                            None => t_max_b.append_null(),
                        }
                    }

                    let batch = try_arrow!(
                        &tx,
                        RecordBatch::try_new(
                            schema.clone(),
                            vec![
                                value_col,
                                Arc::new(UInt8Array::from(zs[offset..end].to_vec())),
                                Arc::new(arrow::array::Int32Array::from(
                                    f_mins[offset..end].to_vec()
                                )),
                                Arc::new(arrow::array::Int32Array::from(
                                    f_maxs[offset..end].to_vec()
                                )),
                                Arc::new(arrow::array::UInt32Array::from(
                                    x_mins[offset..end].to_vec()
                                )),
                                Arc::new(arrow::array::UInt32Array::from(
                                    x_maxs[offset..end].to_vec()
                                )),
                                Arc::new(arrow::array::UInt32Array::from(
                                    y_mins[offset..end].to_vec()
                                )),
                                Arc::new(arrow::array::UInt32Array::from(
                                    y_maxs[offset..end].to_vec()
                                )),
                                Arc::new(is.finish()),
                                Arc::new(t_min_b.finish()),
                                Arc::new(t_max_b.finish()),
                            ],
                        ),
                        "record batch build failed"
                    );

                    if writer.write(&batch).is_err() {
                        return;
                    }
                    offset = end;
                }

                if let Err(e) = writer.finish() {
                    fail_stream(&tx, format!("arrow stream finish failed: {e}"));
                }
            });
        }
        OutputFormat::FlexId => {
            let mut value_refs: Vec<u32> = Vec::new();
            let mut f_zs: Vec<u8> = Vec::new();
            let mut f_is: Vec<i32> = Vec::new();
            let mut x_zs: Vec<u8> = Vec::new();
            let mut x_is: Vec<u32> = Vec::new();
            let mut y_zs: Vec<u8> = Vec::new();
            let mut y_is: Vec<u32> = Vec::new();
            let mut t_zs: Vec<Option<u8>> = Vec::new();
            let mut t_is: Vec<Option<u64>> = Vec::new();

            'groups: for (value, flex_ids) in groups {
                if limit_left == Some(0) {
                    break;
                }
                let start = f_zs.len();
                for flex_id in flex_ids {
                    if !take_one(&mut limit_left) {
                        break;
                    }
                    f_zs.push(flex_id.f_zoomlevel());
                    f_is.push(flex_id.f_index());
                    x_zs.push(flex_id.x_zoomlevel());
                    x_is.push(flex_id.x_index());
                    y_zs.push(flex_id.y_zoomlevel());
                    y_is.push(flex_id.y_index());
                    if flex_id.is_whole_time() {
                        t_zs.push(None);
                        t_is.push(None);
                    } else {
                        t_zs.push(Some(flex_id.t_zoomlevel()));
                        t_is.push(Some(flex_id.t()));
                    }
                }
                // 先に push すると、limit で data に載らなかった値が孤立した辞書項目として残る。
                if f_zs.len() == start {
                    continue 'groups;
                }
                let value_ref = dictionary.len() as u32;
                dictionary.push(to_json(&value)?);
                value_refs.resize(f_zs.len(), value_ref);
            }

            let schema = Arc::new(Schema::new(vec![
                Field::new(
                    "value",
                    build_dictionary_array(&dictionary, vec![])
                        .map_err(|e| AppError::InternalError(e.to_string()))?
                        .data_type()
                        .clone(),
                    true,
                ),
                Field::new("fZoomlevel", DataType::UInt8, false),
                Field::new("fIndex", DataType::Int32, false),
                Field::new("xZoomlevel", DataType::UInt8, false),
                Field::new("xIndex", DataType::UInt32, false),
                Field::new("yZoomlevel", DataType::UInt8, false),
                Field::new("yIndex", DataType::UInt32, false),
                Field::new("tZoomlevel", DataType::UInt8, true),
                Field::new("tIndex", DataType::UInt64, true),
            ]));

            tokio::task::spawn_blocking(move || {
                let mut writer_sink = ChannelWriter { tx: tx.clone() };
                let mut writer = try_arrow!(
                    &tx,
                    StreamWriter::try_new(&mut writer_sink, &schema),
                    "arrow stream init failed"
                );

                let total = f_zs.len();
                let mut offset = 0;
                while offset < total {
                    let end = (offset + ARROW_BATCH_ROWS).min(total);

                    let value_col = try_arrow!(
                        &tx,
                        build_dictionary_array(&dictionary, value_refs[offset..end].to_vec()),
                        "dictionary array build failed"
                    );

                    let mut t_z = UInt8Builder::new();
                    let mut t_i = UInt64Builder::new();
                    for i in offset..end {
                        match t_zs[i] {
                            Some(v) => t_z.append_value(v),
                            None => t_z.append_null(),
                        }
                        match t_is[i] {
                            Some(v) => t_i.append_value(v),
                            None => t_i.append_null(),
                        }
                    }

                    let batch = try_arrow!(
                        &tx,
                        RecordBatch::try_new(
                            schema.clone(),
                            vec![
                                value_col,
                                Arc::new(UInt8Array::from(f_zs[offset..end].to_vec())),
                                Arc::new(arrow::array::Int32Array::from(
                                    f_is[offset..end].to_vec()
                                )),
                                Arc::new(UInt8Array::from(x_zs[offset..end].to_vec())),
                                Arc::new(arrow::array::UInt32Array::from(
                                    x_is[offset..end].to_vec()
                                )),
                                Arc::new(UInt8Array::from(y_zs[offset..end].to_vec())),
                                Arc::new(arrow::array::UInt32Array::from(
                                    y_is[offset..end].to_vec()
                                )),
                                Arc::new(t_z.finish()),
                                Arc::new(t_i.finish()),
                            ],
                        ),
                        "record batch build failed"
                    );

                    if writer.write(&batch).is_err() {
                        return;
                    }
                    offset = end;
                }

                if let Err(e) = writer.finish() {
                    fail_stream(&tx, format!("arrow stream finish failed: {e}"));
                }
            });
        }
    }

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apache.arrow.stream")
        .body(axum::body::Body::from_stream(stream))
        .unwrap()
        .into_response())
}
