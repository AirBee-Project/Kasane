//! `SpatialId` / 動的値 (`TypedValue`) / データ検索レスポンスの相互変換。

use tonic::Status;

use super::convert::enum_from_i32;
use super::pb;
use crate::models::ValueLiteral;
use crate::models::database::table::data::{
    DataGroup, GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
    OutputFormat as DomainOutputFormat, ZoomLevelPolicy as DomainZoomLevelPolicy,
};
use kasane_logic::{
    AllowedIntervals, FlexId, Interval, RangeId, SingleId, SpatialId as _, ZoomLevel,
};

use crate::models::spatial_id::SpatialId as DomainSpatialId;

impl From<pb::ZoomLevelPolicy> for DomainZoomLevelPolicy {
    fn from(value: pb::ZoomLevelPolicy) -> Self {
        match value {
            pb::ZoomLevelPolicy::Ignore => Self::Ignore,
            pb::ZoomLevelPolicy::Normalize => Self::Normalize,
            pb::ZoomLevelPolicy::Unspecified | pb::ZoomLevelPolicy::Error => Self::Error,
        }
    }
}

impl From<i32> for DomainZoomLevelPolicy {
    fn from(value: i32) -> Self {
        enum_from_i32(value, pb::ZoomLevelPolicy::Unspecified).into()
    }
}

impl From<pb::OutputFormat> for DomainOutputFormat {
    fn from(value: pb::OutputFormat) -> Self {
        match value {
            pb::OutputFormat::SingleId => Self::SingleId,
            pb::OutputFormat::FlexId => Self::FlexId,
            pb::OutputFormat::Unspecified | pb::OutputFormat::RangeId => Self::RangeId,
        }
    }
}

impl From<i32> for DomainOutputFormat {
    fn from(value: i32) -> Self {
        enum_from_i32(value, pb::OutputFormat::Unspecified).into()
    }
}

impl From<ValueLiteral> for pb::TypedValue {
    fn from(value: ValueLiteral) -> Self {
        use pb::typed_value::Kind;
        let kind = match value {
            ValueLiteral::Null => Kind::NullVal(0),
            ValueLiteral::Bool(b) => Kind::BoolVal(b),
            ValueLiteral::Int(i) => Kind::IntVal(i),
            ValueLiteral::String(s) => Kind::StringVal(s),
        };
        Self { kind: Some(kind) }
    }
}

impl From<pb::TypedValue> for ValueLiteral {
    fn from(value: pb::TypedValue) -> Self {
        use pb::typed_value::Kind;
        match value.kind {
            None | Some(Kind::NullVal(_)) => Self::Null,
            Some(Kind::BoolVal(b)) => Self::Bool(b),
            Some(Kind::IntVal(i)) => Self::Int(i),
            Some(Kind::StringVal(s)) => Self::String(s),
        }
    }
}

fn invalid_spatial_id(reason: impl Into<String>) -> Status {
    crate::error::AppError::InvalidSpatialId {
        reason: reason.into(),
    }
    .into()
}

impl From<SingleId> for pb::SingleId {
    fn from(id: SingleId) -> Self {
        let (i, t) = if id.is_whole_time() {
            (None, None)
        } else {
            (Some(id.time_interval().seconds()), Some(id.t()))
        };
        Self {
            z: id.z() as u32,
            f: id.f(),
            x: id.x(),
            y: id.y(),
            i,
            t,
        }
    }
}

impl TryFrom<pb::SingleId> for SingleId {
    type Error = Status;

    fn try_from(id: pb::SingleId) -> Result<Self, Self::Error> {
        let z = u8::try_from(id.z).map_err(|_| invalid_spatial_id("z must fit in u8"))?;
        let single =
            SingleId::new(z, id.f, id.x, id.y).map_err(|e| invalid_spatial_id(e.to_string()))?;
        match (id.i, id.t) {
            (None, None) => Ok(single),
            (Some(i), Some(t)) => {
                let interval = Interval::new(i).map_err(|e| invalid_spatial_id(e.to_string()))?;
                if !AllowedIntervals::calendar().contains(interval) {
                    let allowed = AllowedIntervals::calendar()
                        .iter()
                        .map(|unit| unit.seconds().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(invalid_spatial_id(format!(
                        "interval {i} is not allowed; i must be one of: {allowed}"
                    )));
                }
                single
                    .with_time(interval, t)
                    .map_err(|e| invalid_spatial_id(e.to_string()))
            }
            (Some(_), None) => Err(invalid_spatial_id("t must be provided when i is provided")),
            (None, Some(_)) => Err(invalid_spatial_id("i must be provided when t is provided")),
        }
    }
}

impl From<RangeId> for pb::RangeId {
    fn from(id: RangeId) -> Self {
        let (i, t) = if id.is_whole_time() {
            (None, None)
        } else {
            let t_range = id.t();
            (
                Some(id.time_interval().seconds()),
                Some(pb::Uint64Range {
                    min: t_range[0],
                    max: t_range[1],
                }),
            )
        };
        let f = id.f();
        let x = id.x();
        let y = id.y();
        Self {
            z: id.z() as u32,
            f: Some(pb::Int32Range {
                min: f[0],
                max: f[1],
            }),
            x: Some(pb::Uint32Range {
                min: x[0],
                max: x[1],
            }),
            y: Some(pb::Uint32Range {
                min: y[0],
                max: y[1],
            }),
            i,
            t,
        }
    }
}

impl TryFrom<pb::RangeId> for RangeId {
    type Error = Status;

    fn try_from(id: pb::RangeId) -> Result<Self, Self::Error> {
        let z = u8::try_from(id.z).map_err(|_| invalid_spatial_id("z must fit in u8"))?;
        let zoom = ZoomLevel::new(z).map_err(|e| invalid_spatial_id(e.to_string()))?;
        let f =
            id.f.map(|r| [r.min, r.max])
                .unwrap_or([zoom.f_min(), zoom.f_max()]);
        let x = id.x.map(|r| [r.min, r.max]).unwrap_or([0, zoom.xy_max()]);
        let y = id.y.map(|r| [r.min, r.max]).unwrap_or([0, zoom.xy_max()]);
        let range = RangeId::new(z, f, x, y).map_err(|e| invalid_spatial_id(e.to_string()))?;

        match (id.i, id.t) {
            (None, None) => Ok(range),
            (Some(i), Some(t)) => {
                let interval = Interval::new(i).map_err(|e| invalid_spatial_id(e.to_string()))?;
                if !AllowedIntervals::calendar().contains(interval) {
                    let allowed = AllowedIntervals::calendar()
                        .iter()
                        .map(|unit| unit.seconds().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(invalid_spatial_id(format!(
                        "interval {i} is not allowed; i must be one of: {allowed}"
                    )));
                }
                range
                    .with_time(interval, [t.min, t.max])
                    .map_err(|e| invalid_spatial_id(e.to_string()))
            }
            (Some(_), None) => Err(invalid_spatial_id("t must be provided when i is provided")),
            (None, Some(_)) => Err(invalid_spatial_id("i must be provided when t is provided")),
        }
    }
}

impl From<FlexId> for pb::FlexId {
    fn from(id: FlexId) -> Self {
        let (t_zoomlevel, t_index) = if id.is_whole_time() {
            (None, None)
        } else {
            (Some(id.t_zoomlevel() as u32), Some(id.t()))
        };
        Self {
            f_zoomlevel: id.f_zoomlevel() as u32,
            f_index: id.f_index(),
            x_zoomlevel: id.x_zoomlevel() as u32,
            x_index: id.x_index(),
            y_zoomlevel: id.y_zoomlevel() as u32,
            y_index: id.y_index(),
            t_zoomlevel,
            t_index,
        }
    }
}

impl TryFrom<pb::FlexId> for FlexId {
    type Error = Status;

    fn try_from(id: pb::FlexId) -> Result<Self, Self::Error> {
        let fz = u8::try_from(id.f_zoomlevel)
            .map_err(|_| invalid_spatial_id("f_zoomlevel must fit in u8"))?;
        let xz = u8::try_from(id.x_zoomlevel)
            .map_err(|_| invalid_spatial_id("x_zoomlevel must fit in u8"))?;
        let yz = u8::try_from(id.y_zoomlevel)
            .map_err(|_| invalid_spatial_id("y_zoomlevel must fit in u8"))?;

        let flex = FlexId::new(fz, id.f_index, xz, id.x_index, yz, id.y_index)
            .map_err(|e| invalid_spatial_id(e.to_string()))?;

        match (id.t_zoomlevel, id.t_index) {
            (None, None) => Ok(flex),
            (Some(tz), Some(ti)) => {
                let tz_u8 = u8::try_from(tz)
                    .map_err(|_| invalid_spatial_id("t_zoomlevel must fit in u8"))?;
                flex.with_time(tz_u8, ti)
                    .map_err(|e| invalid_spatial_id(e.to_string()))
            }
            (Some(_), None) => Err(invalid_spatial_id(
                "t_index must be provided when t_zoomlevel is provided",
            )),
            (None, Some(_)) => Err(invalid_spatial_id(
                "t_zoomlevel must be provided when t_index is provided",
            )),
        }
    }
}

impl TryFrom<pb::SpatialId> for DomainSpatialId {
    type Error = Status;

    fn try_from(id: pb::SpatialId) -> Result<Self, Self::Error> {
        match id
            .kind
            .ok_or_else(|| invalid_spatial_id("spatial_id.kind must be set"))?
        {
            pb::spatial_id::Kind::SingleId(s) => Ok(DomainSpatialId::SingleId(s.try_into()?)),
            pb::spatial_id::Kind::RangeId(r) => Ok(DomainSpatialId::RangeId(r.try_into()?)),
            pb::spatial_id::Kind::FlexId(f) => Ok(DomainSpatialId::FlexId(f.try_into()?)),
        }
    }
}

impl From<DomainSpatialId> for pb::SpatialId {
    fn from(id: DomainSpatialId) -> Self {
        match id {
            DomainSpatialId::SingleId(s) => pb::SpatialId {
                kind: Some(pb::spatial_id::Kind::SingleId(s.into())),
            },
            DomainSpatialId::RangeId(r) => pb::SpatialId {
                kind: Some(pb::spatial_id::Kind::RangeId(r.into())),
            },
            DomainSpatialId::FlexId(f) => pb::SpatialId {
                kind: Some(pb::spatial_id::Kind::FlexId(f.into())),
            },
        }
    }
}

fn data_group_to_pb<T>(
    group: DataGroup<T>,
    id_to_pb: impl Fn(T) -> pb::SpatialId,
) -> pb::DataGroup {
    pb::DataGroup {
        value_ref: group.value_ref as u64,
        spatial_ids: group.spatial_ids.into_iter().map(id_to_pb).collect(),
    }
}

impl From<GetDataResponse> for pb::SearchDataResponse {
    fn from(response: GetDataResponse) -> Self {
        match response {
            GetDataResponse::Single(GetDataResponseSingle { dictionary, data }) => {
                pb::SearchDataResponse {
                    dictionary: dictionary.into_iter().map(Into::into).collect(),
                    data: data
                        .into_iter()
                        .map(|g| {
                            data_group_to_pb(g, |id| pb::SpatialId {
                                kind: Some(pb::spatial_id::Kind::SingleId(id.into())),
                            })
                        })
                        .collect(),
                }
            }
            GetDataResponse::Range(GetDataResponseRange { dictionary, data }) => {
                pb::SearchDataResponse {
                    dictionary: dictionary.into_iter().map(Into::into).collect(),
                    data: data
                        .into_iter()
                        .map(|g| {
                            data_group_to_pb(g, |id| pb::SpatialId {
                                kind: Some(pb::spatial_id::Kind::RangeId(id.into())),
                            })
                        })
                        .collect(),
                }
            }
            GetDataResponse::Flex(GetDataResponseFlex { dictionary, data }) => {
                pb::SearchDataResponse {
                    dictionary: dictionary.into_iter().map(Into::into).collect(),
                    data: data
                        .into_iter()
                        .map(|g| {
                            data_group_to_pb(g, |id| pb::SpatialId {
                                kind: Some(pb::spatial_id::Kind::FlexId(id.into())),
                            })
                        })
                        .collect(),
                }
            }
        }
    }
}

/// ストリーミング送信時の 1 チャンクあたりの既定の空間ID数。
pub const DEFAULT_CHUNK_SIZE: usize = 2000;

fn chunk_groups<T>(
    dictionary: &[ValueLiteral],
    data: Vec<DataGroup<T>>,
    chunk_size: usize,
    id_to_pb: impl Fn(T) -> pb::SpatialId,
) -> Vec<pb::SearchDataResponse> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current_dict_values: Vec<pb::TypedValue> = Vec::new();
    let mut orig_to_chunk_dict: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    let mut current_data: Vec<pb::DataGroup> = Vec::new();
    let mut current_count = 0;

    for group in data {
        let orig_ref = group.value_ref;
        let mut ids_iter = group.spatial_ids.into_iter();

        loop {
            let remaining_capacity = chunk_size.saturating_sub(current_count);
            if remaining_capacity == 0 && current_count > 0 {
                chunks.push(pb::SearchDataResponse {
                    dictionary: std::mem::take(&mut current_dict_values),
                    data: std::mem::take(&mut current_data),
                });
                orig_to_chunk_dict.clear();
                current_count = 0;
                continue;
            }

            let batch: Vec<_> = ids_iter
                .by_ref()
                .take(if remaining_capacity > 0 {
                    remaining_capacity
                } else {
                    chunk_size
                })
                .collect();
            if batch.is_empty() {
                break;
            }

            let batch_len = batch.len();
            let chunk_ref = *orig_to_chunk_dict.entry(orig_ref).or_insert_with(|| {
                let idx = current_dict_values.len();
                let val_literal = dictionary
                    .get(orig_ref)
                    .cloned()
                    .unwrap_or(ValueLiteral::Null);
                current_dict_values.push(val_literal.into());
                idx
            });

            current_data.push(pb::DataGroup {
                value_ref: chunk_ref as u64,
                spatial_ids: batch.into_iter().map(&id_to_pb).collect(),
            });
            current_count += batch_len;
        }
    }

    if !current_data.is_empty() {
        chunks.push(pb::SearchDataResponse {
            dictionary: current_dict_values,
            data: current_data,
        });
    }

    chunks
}

/// [`GetDataResponse`] を指定したチャンクサイズごとの [`pb::SearchDataResponse`] に分割する。
pub fn data_response_to_chunks(
    response: GetDataResponse,
    chunk_size: usize,
) -> Vec<pb::SearchDataResponse> {
    let chunk_size = if chunk_size == 0 {
        DEFAULT_CHUNK_SIZE
    } else {
        chunk_size
    };
    match response {
        GetDataResponse::Single(GetDataResponseSingle { dictionary, data }) => {
            chunk_groups(&dictionary, data, chunk_size, |id| pb::SpatialId {
                kind: Some(pb::spatial_id::Kind::SingleId(id.into())),
            })
        }
        GetDataResponse::Range(GetDataResponseRange { dictionary, data }) => {
            chunk_groups(&dictionary, data, chunk_size, |id| pb::SpatialId {
                kind: Some(pb::spatial_id::Kind::RangeId(id.into())),
            })
        }
        GetDataResponse::Flex(GetDataResponseFlex { dictionary, data }) => {
            chunk_groups(&dictionary, data, chunk_size, |id| pb::SpatialId {
                kind: Some(pb::spatial_id::Kind::FlexId(id.into())),
            })
        }
    }
}
