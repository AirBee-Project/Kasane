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
use axum::body::Body;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

const CHUNK_SIZE: usize = 65536;

struct ChannelWriter {
    tx: mpsc::Sender<Result<axum::body::Bytes, std::convert::Infallible>>,
    buffer: Vec<u8>,
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        if self.buffer.len() >= 64 * 1024 {
            self.flush()?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            let bytes = std::mem::take(&mut self.buffer);
            if self.tx.blocking_send(Ok(bytes.into())).is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "channel closed",
                ));
            }
        }
        Ok(())
    }
}

pub fn stream_arrow_ipc(response: GetDataResponse) -> Body {
    let (tx, rx) = mpsc::channel(4);

    tokio::task::spawn_blocking(move || {
        let channel_writer = ChannelWriter {
            tx: tx.clone(),
            buffer: Vec::with_capacity(64 * 1024),
        };

        let result = match response {
            GetDataResponse::Single(r) => stream_batch_single(r, channel_writer),
            GetDataResponse::Range(r) => stream_batch_range(r, channel_writer),
            GetDataResponse::Flex(r) => stream_batch_flex(r, channel_writer),
        };

        if let Err(e) = result {
            tracing::error!("Failed to encode Arrow IPC stream: {}", e);
        }
    });

    Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
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

fn stream_batch_single(
    r: GetDataResponseSingle,
    mut channel_writer: ChannelWriter,
) -> Result<(), arrow::error::ArrowError> {
    let value_type = get_dictionary_type(&r.dictionary);
    let schema = Arc::new(Schema::new(vec![
        Field::new("value", value_type, true),
        Field::new("z", DataType::UInt8, false),
        Field::new("f", DataType::Int32, false),
        Field::new("x", DataType::UInt32, false),
        Field::new("y", DataType::UInt32, false),
        Field::new("i", DataType::UInt64, true),
        Field::new("t", DataType::UInt64, true),
    ]));

    let mut writer = StreamWriter::try_new(&mut channel_writer, &schema)?;

    // Iterator over all spatial IDs with their value_ref
    let iter = r.data.into_iter().flat_map(|group| {
        let vref = group.value_ref as u32;
        group.spatial_ids.into_iter().map(move |sid| (vref, sid))
    });

    use itertools::Itertools;
    for chunk in &iter.chunks(CHUNK_SIZE) {
        let mut value_refs = Vec::with_capacity(CHUNK_SIZE);
        let mut zs = Vec::with_capacity(CHUNK_SIZE);
        let mut fs = Vec::with_capacity(CHUNK_SIZE);
        let mut xs = Vec::with_capacity(CHUNK_SIZE);
        let mut ys = Vec::with_capacity(CHUNK_SIZE);
        let mut is = UInt64Builder::with_capacity(CHUNK_SIZE);
        let mut ts = UInt64Builder::with_capacity(CHUNK_SIZE);

        for (vref, sid) in chunk {
            value_refs.push(vref);
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

        let value_col = build_dictionary_array(&r.dictionary, value_refs)?;

        let batch = RecordBatch::try_new(
            schema.clone(),
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

        writer.write(&batch)?;
    }

    writer.finish()?;
    drop(writer);
    std::io::Write::flush(&mut channel_writer)?;
    Ok(())
}

fn stream_batch_range(
    r: GetDataResponseRange,
    mut channel_writer: ChannelWriter,
) -> Result<(), arrow::error::ArrowError> {
    let value_type = get_dictionary_type(&r.dictionary);
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

    let mut writer = StreamWriter::try_new(&mut channel_writer, &schema)?;

    let iter = r.data.into_iter().flat_map(|group| {
        let vref = group.value_ref as u32;
        group.spatial_ids.into_iter().map(move |sid| (vref, sid))
    });

    use itertools::Itertools;
    for chunk in &iter.chunks(CHUNK_SIZE) {
        let mut value_refs = Vec::with_capacity(CHUNK_SIZE);
        let mut zs = Vec::with_capacity(CHUNK_SIZE);
        let mut f_mins = Int32Builder::with_capacity(CHUNK_SIZE);
        let mut f_maxs = Int32Builder::with_capacity(CHUNK_SIZE);
        let mut x_mins = UInt32Builder::with_capacity(CHUNK_SIZE);
        let mut x_maxs = UInt32Builder::with_capacity(CHUNK_SIZE);
        let mut y_mins = UInt32Builder::with_capacity(CHUNK_SIZE);
        let mut y_maxs = UInt32Builder::with_capacity(CHUNK_SIZE);
        let mut is = UInt64Builder::with_capacity(CHUNK_SIZE);
        let mut t_mins = UInt64Builder::with_capacity(CHUNK_SIZE);
        let mut t_maxs = UInt64Builder::with_capacity(CHUNK_SIZE);

        for (vref, sid) in chunk {
            value_refs.push(vref);
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

        let value_col = build_dictionary_array(&r.dictionary, value_refs)?;

        let batch = RecordBatch::try_new(
            schema.clone(),
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

        writer.write(&batch)?;
    }

    writer.finish()?;
    drop(writer);
    std::io::Write::flush(&mut channel_writer)?;
    Ok(())
}

fn stream_batch_flex(
    r: GetDataResponseFlex,
    mut channel_writer: ChannelWriter,
) -> Result<(), arrow::error::ArrowError> {
    let value_type = get_dictionary_type(&r.dictionary);
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

    let mut writer = StreamWriter::try_new(&mut channel_writer, &schema)?;

    let iter = r.data.into_iter().flat_map(|group| {
        let vref = group.value_ref as u32;
        group.spatial_ids.into_iter().map(move |sid| (vref, sid))
    });

    use itertools::Itertools;
    for chunk in &iter.chunks(CHUNK_SIZE) {
        let mut value_refs = Vec::with_capacity(CHUNK_SIZE);
        let mut f_zs = Vec::with_capacity(CHUNK_SIZE);
        let mut f_is = Vec::with_capacity(CHUNK_SIZE);
        let mut x_zs = Vec::with_capacity(CHUNK_SIZE);
        let mut x_is = Vec::with_capacity(CHUNK_SIZE);
        let mut y_zs = Vec::with_capacity(CHUNK_SIZE);
        let mut y_is = Vec::with_capacity(CHUNK_SIZE);
        let mut t_zs = UInt8Builder::with_capacity(CHUNK_SIZE);
        let mut t_is = UInt64Builder::with_capacity(CHUNK_SIZE);

        for (vref, sid) in chunk {
            value_refs.push(vref);
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

        let value_col = build_dictionary_array(&r.dictionary, value_refs)?;

        let batch = RecordBatch::try_new(
            schema.clone(),
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

        writer.write(&batch)?;
    }

    writer.finish()?;
    drop(writer);
    std::io::Write::flush(&mut channel_writer)?;
    Ok(())
}

fn get_dictionary_type(dictionary: &[Value]) -> DataType {
    let is_numeric = dictionary.iter().all(|v| v.is_number() || v.is_null());
    let value_type = if is_numeric {
        DataType::Float64
    } else {
        DataType::Utf8
    };
    DataType::Dictionary(Box::new(DataType::UInt32), Box::new(value_type))
}
