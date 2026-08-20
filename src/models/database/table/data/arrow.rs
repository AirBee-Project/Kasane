use axum::response::Response;
use bytes::Bytes;
use kasane_logic::{AllowedIntervals, FlexId, SpatialId as _, SpatialIdSet};
use serde_json::Value;

use super::stream::build_stream_response;
use crate::{
    error::AppError,
    models::database::table::{
        TableDataType,
        data::{
            GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
            OutputFormat,
        },
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

/// ストリーミング応答の value 列の Arrow 型を、テーブルの `data_type` から直接決める。
///
/// `build_dictionary_array` の分類（数値/真偽値/文字列）は実際には常に `data_type` だけで
/// 決まる（`V::to_json` は型ごとに常に同じ JSON 種別を返すため）。データを見ずに決められる
/// ので、スキーマは行を1件も処理する前に確定できる — これがストリーミングの前提になる。
fn dictionary_value_arrow_type(data_type: TableDataType) -> DataType {
    match data_type {
        TableDataType::Int => DataType::Float64,
        TableDataType::Text | TableDataType::Enum => DataType::Utf8,
        TableDataType::Boolean => DataType::Boolean,
        // Presence の値は常に JSON null。all_numbers は is_null() も許すため、全 null の辞書は
        // 今まで通り Float64 として扱う。
        TableDataType::Presence => DataType::Float64,
    }
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

/// `spawn_blocking` の中身を panic から守る。panic すると `tx` がそのまま drop され、
/// `fail_stream` が呼ばれないまま無言でストリームが切れてしまう（`Result` を返す失敗は
/// 上の `try_arrow!`/`fail_stream` で拾えるが、panic はそれをすり抜ける）。
fn spawn_blocking_stream(
    tx: tokio::sync::mpsc::Sender<Result<Bytes, String>>,
    body: impl FnOnce(tokio::sync::mpsc::Sender<Result<Bytes, String>>) + Send + 'static,
) {
    tokio::task::spawn_blocking(move || {
        let panic_tx = tx.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(tx)));
        if result.is_err() {
            fail_stream(&panic_tx, "internal panic while generating stream");
        }
    });
}

/// `OutputFormat` ごとに異なる列（value 列を除く）の蓄積とバッチ変換を切り出したもの。
/// バッチ分割・辞書管理・書き込み・panic 保護は `stream_format` 側の共通処理。
trait FormatBuffers: Default {
    /// 1 group の `flex_ids` を `limit_left` の予算内で展開し、積んだ行数を返す
    /// （0 なら group は不採用— 呼び出し側は辞書に登録しない）。
    fn push_group(&mut self, flex_ids: Vec<FlexId>, limit_left: &mut Option<usize>) -> usize;
    /// value 列を除く Field 定義。
    fn fields() -> Vec<Field>;
    /// `[offset, end)` の範囲を value 列を除く Array 群に変換する。
    fn columns(&self, offset: usize, end: usize) -> Vec<Arc<dyn arrow::array::Array>>;
    fn row_count(&self) -> usize;
}

#[derive(Default)]
struct SingleIdBuffers {
    zs: Vec<u8>,
    fs: Vec<i32>,
    xs: Vec<u32>,
    ys: Vec<u32>,
    is_vals: Vec<Option<u64>>,
    ts_vals: Vec<Option<u64>>,
}

impl FormatBuffers for SingleIdBuffers {
    fn push_group(&mut self, flex_ids: Vec<FlexId>, limit_left: &mut Option<usize>) -> usize {
        let set: SpatialIdSet = flex_ids.into_iter().collect();
        let start = self.zs.len();
        'ranges: for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
            for single_id in range_id.single_ids() {
                if !take_one(limit_left) {
                    break 'ranges;
                }
                self.zs.push(single_id.z());
                self.fs.push(single_id.f());
                self.xs.push(single_id.x());
                self.ys.push(single_id.y());
                if single_id.is_whole_time() {
                    self.is_vals.push(None);
                    self.ts_vals.push(None);
                } else {
                    self.is_vals.push(Some(single_id.time_interval().seconds()));
                    self.ts_vals.push(Some(single_id.t()));
                }
            }
        }
        self.zs.len() - start
    }

    fn fields() -> Vec<Field> {
        vec![
            Field::new("z", DataType::UInt8, false),
            Field::new("f", DataType::Int32, false),
            Field::new("x", DataType::UInt32, false),
            Field::new("y", DataType::UInt32, false),
            Field::new("i", DataType::UInt64, true),
            Field::new("t", DataType::UInt64, true),
        ]
    }

    fn columns(&self, offset: usize, end: usize) -> Vec<Arc<dyn arrow::array::Array>> {
        let mut is = UInt64Builder::new();
        let mut ts = UInt64Builder::new();
        for i in offset..end {
            match self.is_vals[i] {
                Some(v) => is.append_value(v),
                None => is.append_null(),
            }
            match self.ts_vals[i] {
                Some(v) => ts.append_value(v),
                None => ts.append_null(),
            }
        }
        vec![
            Arc::new(UInt8Array::from(self.zs[offset..end].to_vec())),
            Arc::new(arrow::array::Int32Array::from(
                self.fs[offset..end].to_vec(),
            )),
            Arc::new(arrow::array::UInt32Array::from(
                self.xs[offset..end].to_vec(),
            )),
            Arc::new(arrow::array::UInt32Array::from(
                self.ys[offset..end].to_vec(),
            )),
            Arc::new(is.finish()),
            Arc::new(ts.finish()),
        ]
    }

    fn row_count(&self) -> usize {
        self.zs.len()
    }
}

#[derive(Default)]
struct RangeIdBuffers {
    zs: Vec<u8>,
    f_mins: Vec<i32>,
    f_maxs: Vec<i32>,
    x_mins: Vec<u32>,
    x_maxs: Vec<u32>,
    y_mins: Vec<u32>,
    y_maxs: Vec<u32>,
    is_vals: Vec<Option<u64>>,
    t_mins: Vec<Option<u64>>,
    t_maxs: Vec<Option<u64>>,
}

impl FormatBuffers for RangeIdBuffers {
    fn push_group(&mut self, flex_ids: Vec<FlexId>, limit_left: &mut Option<usize>) -> usize {
        let set: SpatialIdSet = flex_ids.into_iter().collect();
        let start = self.zs.len();
        for range_id in set.range_ids_in(AllowedIntervals::calendar()) {
            if !take_one(limit_left) {
                break;
            }
            self.zs.push(range_id.z());
            self.f_mins.push(range_id.f()[0]);
            self.f_maxs.push(range_id.f()[1]);
            self.x_mins.push(range_id.x()[0]);
            self.x_maxs.push(range_id.x()[1]);
            self.y_mins.push(range_id.y()[0]);
            self.y_maxs.push(range_id.y()[1]);
            if range_id.is_whole_time() {
                self.is_vals.push(None);
                self.t_mins.push(None);
                self.t_maxs.push(None);
            } else {
                self.is_vals.push(Some(range_id.time_interval().seconds()));
                self.t_mins.push(Some(range_id.t()[0]));
                self.t_maxs.push(Some(range_id.t()[1]));
            }
        }
        self.zs.len() - start
    }

    fn fields() -> Vec<Field> {
        vec![
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
        ]
    }

    fn columns(&self, offset: usize, end: usize) -> Vec<Arc<dyn arrow::array::Array>> {
        let mut is = UInt64Builder::new();
        let mut t_min_b = UInt64Builder::new();
        let mut t_max_b = UInt64Builder::new();
        for i in offset..end {
            match self.is_vals[i] {
                Some(v) => is.append_value(v),
                None => is.append_null(),
            }
            match self.t_mins[i] {
                Some(v) => t_min_b.append_value(v),
                None => t_min_b.append_null(),
            }
            match self.t_maxs[i] {
                Some(v) => t_max_b.append_value(v),
                None => t_max_b.append_null(),
            }
        }
        vec![
            Arc::new(UInt8Array::from(self.zs[offset..end].to_vec())),
            Arc::new(arrow::array::Int32Array::from(
                self.f_mins[offset..end].to_vec(),
            )),
            Arc::new(arrow::array::Int32Array::from(
                self.f_maxs[offset..end].to_vec(),
            )),
            Arc::new(arrow::array::UInt32Array::from(
                self.x_mins[offset..end].to_vec(),
            )),
            Arc::new(arrow::array::UInt32Array::from(
                self.x_maxs[offset..end].to_vec(),
            )),
            Arc::new(arrow::array::UInt32Array::from(
                self.y_mins[offset..end].to_vec(),
            )),
            Arc::new(arrow::array::UInt32Array::from(
                self.y_maxs[offset..end].to_vec(),
            )),
            Arc::new(is.finish()),
            Arc::new(t_min_b.finish()),
            Arc::new(t_max_b.finish()),
        ]
    }

    fn row_count(&self) -> usize {
        self.zs.len()
    }
}

#[derive(Default)]
struct FlexIdBuffers {
    f_zs: Vec<u8>,
    f_is: Vec<i32>,
    x_zs: Vec<u8>,
    x_is: Vec<u32>,
    y_zs: Vec<u8>,
    y_is: Vec<u32>,
    t_zs: Vec<Option<u8>>,
    t_is: Vec<Option<u64>>,
}

impl FormatBuffers for FlexIdBuffers {
    fn push_group(&mut self, flex_ids: Vec<FlexId>, limit_left: &mut Option<usize>) -> usize {
        let start = self.f_zs.len();
        for flex_id in flex_ids {
            if !take_one(limit_left) {
                break;
            }
            self.f_zs.push(flex_id.f_zoomlevel());
            self.f_is.push(flex_id.f_index());
            self.x_zs.push(flex_id.x_zoomlevel());
            self.x_is.push(flex_id.x_index());
            self.y_zs.push(flex_id.y_zoomlevel());
            self.y_is.push(flex_id.y_index());
            if flex_id.is_whole_time() {
                self.t_zs.push(None);
                self.t_is.push(None);
            } else {
                self.t_zs.push(Some(flex_id.t_zoomlevel()));
                self.t_is.push(Some(flex_id.t()));
            }
        }
        self.f_zs.len() - start
    }

    fn fields() -> Vec<Field> {
        vec![
            Field::new("fZoomlevel", DataType::UInt8, false),
            Field::new("fIndex", DataType::Int32, false),
            Field::new("xZoomlevel", DataType::UInt8, false),
            Field::new("xIndex", DataType::UInt32, false),
            Field::new("yZoomlevel", DataType::UInt8, false),
            Field::new("yIndex", DataType::UInt32, false),
            Field::new("tZoomlevel", DataType::UInt8, true),
            Field::new("tIndex", DataType::UInt64, true),
        ]
    }

    fn columns(&self, offset: usize, end: usize) -> Vec<Arc<dyn arrow::array::Array>> {
        let mut t_z = UInt8Builder::new();
        let mut t_i = UInt64Builder::new();
        for i in offset..end {
            match self.t_zs[i] {
                Some(v) => t_z.append_value(v),
                None => t_z.append_null(),
            }
            match self.t_is[i] {
                Some(v) => t_i.append_value(v),
                None => t_i.append_null(),
            }
        }
        vec![
            Arc::new(UInt8Array::from(self.f_zs[offset..end].to_vec())),
            Arc::new(arrow::array::Int32Array::from(
                self.f_is[offset..end].to_vec(),
            )),
            Arc::new(UInt8Array::from(self.x_zs[offset..end].to_vec())),
            Arc::new(arrow::array::UInt32Array::from(
                self.x_is[offset..end].to_vec(),
            )),
            Arc::new(UInt8Array::from(self.y_zs[offset..end].to_vec())),
            Arc::new(arrow::array::UInt32Array::from(
                self.y_is[offset..end].to_vec(),
            )),
            Arc::new(t_z.finish()),
            Arc::new(t_i.finish()),
        ]
    }

    fn row_count(&self) -> usize {
        self.f_zs.len()
    }
}

/// その時点までに確定している辞書と、行ごとの辞書参照。バッチをまたいで伸びていく。
struct DictionaryState<'a> {
    dictionary: &'a [Value],
    value_refs: &'a [u32],
}

/// 蓄積中の1バッチ分（`[offset, end)`）を書き出す。辞書は「その時点までに確定した全体」を
/// 毎回渡す — Arrow IPC の `StreamWriter`（既定の `DictionaryHandling::Resend`）は辞書の
/// 内容がバッチ間で変わっても自動的に再送してくれるので、辞書をバッチごとに区切って
/// 差分管理する必要はない。
fn flush_batch<B: FormatBuffers, W: std::io::Write>(
    tx: &tokio::sync::mpsc::Sender<Result<Bytes, String>>,
    writer: &mut StreamWriter<W>,
    schema: &Arc<Schema>,
    dict: DictionaryState<'_>,
    buffers: &B,
    offset: usize,
    end: usize,
) -> bool {
    let value_col =
        match build_dictionary_array(dict.dictionary, dict.value_refs[offset..end].to_vec()) {
            Ok(v) => v,
            Err(e) => {
                fail_stream(tx, format!("dictionary array build failed: {e}"));
                return false;
            }
        };

    let mut columns = vec![value_col];
    columns.extend(buffers.columns(offset, end));

    let batch = match RecordBatch::try_new(schema.clone(), columns) {
        Ok(b) => b,
        Err(e) => {
            fail_stream(tx, format!("record batch build failed: {e}"));
            return false;
        }
    };

    writer.write(&batch).is_ok()
}

/// `format` 1 種類ぶんの本体。スキーマは行を処理する前に確定させ（`dictionary_value_arrow_type`
/// が実データを見ずに決められるおかげ）、`ARROW_BATCH_ROWS` 行たまるたびにその場で書き出す
/// ので、ピークメモリは「distinct 値数（辞書）+ 直近バッチぶんの行バッファ」に収まる。
fn stream_format<V, F, B>(
    groups: Vec<(V, Vec<FlexId>)>,
    limit: Option<usize>,
    value_type: TableDataType,
    to_json: F,
) -> Result<Response, AppError>
where
    V: Send + 'static,
    F: Fn(&V) -> Result<Value, AppError> + Send + 'static,
    B: FormatBuffers + Send + 'static,
{
    let mut fields = vec![Field::new(
        "value",
        dictionary_value_arrow_type(value_type),
        true,
    )];
    fields.extend(B::fields());
    let schema = Arc::new(Schema::new(fields));

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, String>>(4);

    let schema_for_task = schema.clone();
    spawn_blocking_stream(tx, move |tx| {
        let mut writer_sink = ChannelWriter { tx: tx.clone() };
        let mut writer = try_arrow!(
            &tx,
            StreamWriter::try_new(&mut writer_sink, &schema_for_task),
            "arrow stream init failed"
        );

        let mut dictionary: Vec<Value> = Vec::new();
        let mut value_refs: Vec<u32> = Vec::new();
        let mut buffers = B::default();
        let mut flushed = 0usize;
        let mut limit_left = limit;

        for (value, flex_ids) in groups {
            if limit_left == Some(0) {
                break;
            }

            let added = buffers.push_group(flex_ids, &mut limit_left);
            if added == 0 {
                // 先に辞書へ push すると、limit で data に載らなかった値が孤立した辞書項目
                // として残る。行を1件も積めなかった group はここで見送る。
                continue;
            }

            let value_json = match to_json(&value) {
                Ok(v) => v,
                Err(e) => {
                    fail_stream(&tx, format!("value to_json failed: {e}"));
                    return;
                }
            };
            let value_ref = dictionary.len() as u32;
            dictionary.push(value_json);
            value_refs.resize(buffers.row_count(), value_ref);

            while buffers.row_count() - flushed >= ARROW_BATCH_ROWS {
                let end = flushed + ARROW_BATCH_ROWS;
                let dict = DictionaryState {
                    dictionary: &dictionary,
                    value_refs: &value_refs,
                };
                if !flush_batch(
                    &tx,
                    &mut writer,
                    &schema_for_task,
                    dict,
                    &buffers,
                    flushed,
                    end,
                ) {
                    return;
                }
                flushed = end;
            }
        }

        let total = buffers.row_count();
        if total > flushed {
            let dict = DictionaryState {
                dictionary: &dictionary,
                value_refs: &value_refs,
            };
            if !flush_batch(
                &tx,
                &mut writer,
                &schema_for_task,
                dict,
                &buffers,
                flushed,
                total,
            ) {
                return;
            }
        }

        if let Err(e) = writer.finish() {
            fail_stream(&tx, format!("arrow stream finish failed: {e}"));
        }
    });

    Ok(build_stream_response(
        rx,
        "application/vnd.apache.arrow.stream",
    ))
}

pub fn stream_arrow_ipc<V, F>(
    groups: Vec<(V, Vec<FlexId>)>,
    format: OutputFormat,
    limit: Option<usize>,
    value_type: TableDataType,
    to_json: F,
) -> Result<Response, AppError>
where
    V: Send + 'static,
    F: Fn(&V) -> Result<Value, AppError> + Send + 'static,
{
    match format {
        OutputFormat::SingleId => {
            stream_format::<V, F, SingleIdBuffers>(groups, limit, value_type, to_json)
        }
        OutputFormat::RangeId => {
            stream_format::<V, F, RangeIdBuffers>(groups, limit, value_type, to_json)
        }
        OutputFormat::FlexId => {
            stream_format::<V, F, FlexIdBuffers>(groups, limit, value_type, to_json)
        }
    }
}
