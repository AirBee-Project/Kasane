//! `SpatialId` / 動的値 (`TypedValue`) / データ検索レスポンスの相互変換。

use tonic::Status;

use super::convert::enum_from_i32;
use super::pb;
use crate::models::database::table::data::{
    DataGroup, GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
    OutputFormat as DomainOutputFormat, ZoomLevelPolicy as DomainZoomLevelPolicy,
};
use crate::models::spatial_id::{RawFlexId, RawRangeId, RawSingleId, SpatialId as DomainSpatialId};

impl From<pb::ZoomLevelPolicy> for DomainZoomLevelPolicy {
    fn from(value: pb::ZoomLevelPolicy) -> Self {
        match value {
            pb::ZoomLevelPolicy::Ignore => Self::Ignore,
            pb::ZoomLevelPolicy::Normalize => Self::Normalize,
            pb::ZoomLevelPolicy::Unspecified | pb::ZoomLevelPolicy::Error => Self::Error,
        }
    }
}

pub fn zoom_level_policy_to_domain(value: i32) -> DomainZoomLevelPolicy {
    enum_from_i32(value, pb::ZoomLevelPolicy::Unspecified).into()
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

pub fn output_format_to_domain(value: i32) -> DomainOutputFormat {
    enum_from_i32(value, pb::OutputFormat::Unspecified).into()
}

pub fn json_to_typed_value(value: serde_json::Value) -> pb::TypedValue {
    use pb::typed_value::Kind;
    let kind = match value {
        serde_json::Value::Null => Kind::NullVal(0),
        serde_json::Value::Bool(b) => Kind::BoolVal(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Kind::IntVal(i)
            } else if let Some(u) = n.as_u64() {
                Kind::IntVal(u as i64)
            } else if let Some(f) = n.as_f64() {
                Kind::IntVal(f as i64)
            } else {
                Kind::NullVal(0)
            }
        }
        serde_json::Value::String(s) => Kind::StringVal(s),
        _ => Kind::NullVal(0),
    };
    pb::TypedValue { kind: Some(kind) }
}

pub fn typed_value_to_json(value: Option<pb::TypedValue>) -> serde_json::Value {
    use pb::typed_value::Kind;
    match value.and_then(|v| v.kind) {
        None | Some(Kind::NullVal(_)) => serde_json::Value::Null,
        Some(Kind::BoolVal(b)) => serde_json::Value::Bool(b),
        Some(Kind::IntVal(i)) => serde_json::Value::Number(i.into()),
        Some(Kind::StringVal(s)) => serde_json::Value::String(s),
    }
}

impl From<pb::SingleId> for RawSingleId {
    fn from(id: pb::SingleId) -> Self {
        Self {
            z: id.z as u8,
            f: id.f,
            x: id.x,
            y: id.y,
            i: id.i,
            t: id.t,
        }
    }
}

impl From<RawSingleId> for pb::SingleId {
    fn from(id: RawSingleId) -> Self {
        Self {
            z: id.z as u32,
            f: id.f,
            x: id.x,
            y: id.y,
            i: id.i,
            t: id.t,
        }
    }
}

impl From<pb::RangeId> for RawRangeId {
    fn from(id: pb::RangeId) -> Self {
        Self {
            z: id.z as u8,
            f: id.f.map(|r| [r.min, r.max]),
            x: id.x.map(|r| [r.min, r.max]),
            y: id.y.map(|r| [r.min, r.max]),
            i: id.i,
            t: id.t.map(|r| [r.min, r.max]),
        }
    }
}

impl From<RawRangeId> for pb::RangeId {
    fn from(id: RawRangeId) -> Self {
        Self {
            z: id.z as u32,
            f: id.f.map(|[min, max]| pb::Int32Range { min, max }),
            x: id.x.map(|[min, max]| pb::Uint32Range { min, max }),
            y: id.y.map(|[min, max]| pb::Uint32Range { min, max }),
            i: id.i,
            t: id.t.map(|[min, max]| pb::Uint64Range { min, max }),
        }
    }
}

impl From<pb::FlexId> for RawFlexId {
    fn from(id: pb::FlexId) -> Self {
        Self {
            f_zoomlevel: id.f_zoomlevel as u8,
            f_index: id.f_index,
            x_zoomlevel: id.x_zoomlevel as u8,
            x_index: id.x_index,
            y_zoomlevel: id.y_zoomlevel as u8,
            y_index: id.y_index,
            t_zoomlevel: id.t_zoomlevel.map(|v| v as u8),
            t_index: id.t_index,
        }
    }
}

impl From<RawFlexId> for pb::FlexId {
    fn from(id: RawFlexId) -> Self {
        Self {
            f_zoomlevel: id.f_zoomlevel as u32,
            f_index: id.f_index,
            x_zoomlevel: id.x_zoomlevel as u32,
            x_index: id.x_index,
            y_zoomlevel: id.y_zoomlevel as u32,
            y_index: id.y_index,
            t_zoomlevel: id.t_zoomlevel.map(|v| v as u32),
            t_index: id.t_index,
        }
    }
}

impl TryFrom<pb::SpatialId> for DomainSpatialId {
    type Error = Status;

    fn try_from(id: pb::SpatialId) -> Result<Self, Self::Error> {
        match id
            .kind
            .ok_or_else(|| Status::invalid_argument("spatial_id.kind must be set"))?
        {
            pb::spatial_id::Kind::SingleId(s) => Ok(DomainSpatialId::SingleId(s.into())),
            pb::spatial_id::Kind::RangeId(r) => Ok(DomainSpatialId::RangeId(r.into())),
            pb::spatial_id::Kind::FlexId(f) => Ok(DomainSpatialId::FlexId(f.into())),
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

pub fn spatial_ids_to_domain(ids: Vec<pb::SpatialId>) -> Result<Vec<DomainSpatialId>, Status> {
    ids.into_iter().map(TryInto::try_into).collect()
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
                    dictionary: dictionary.into_iter().map(json_to_typed_value).collect(),
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
                    dictionary: dictionary.into_iter().map(json_to_typed_value).collect(),
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
                    dictionary: dictionary.into_iter().map(json_to_typed_value).collect(),
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
