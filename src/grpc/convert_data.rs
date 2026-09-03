//! `SpatialId` / 動的値 / データ検索レスポンスの相互変換。

use tonic::Status;

use super::pb;
use crate::models::database::table::data::{
    DataGroup, GetDataResponse, GetDataResponseFlex, GetDataResponseRange, GetDataResponseSingle,
    OutputFormat as DomainOutputFormat, ZoomLevelPolicy as DomainZoomLevelPolicy,
};
use crate::models::spatial_id::{RawFlexId, RawRangeId, RawSingleId, SpatialId as DomainSpatialId};

pub fn zoom_level_policy_to_domain(value: i32) -> DomainZoomLevelPolicy {
    match pb::ZoomLevelPolicy::try_from(value).unwrap_or(pb::ZoomLevelPolicy::Unspecified) {
        pb::ZoomLevelPolicy::Ignore => DomainZoomLevelPolicy::Ignore,
        pb::ZoomLevelPolicy::Normalize => DomainZoomLevelPolicy::Normalize,
        pb::ZoomLevelPolicy::Unspecified | pb::ZoomLevelPolicy::Error => {
            DomainZoomLevelPolicy::Error
        }
    }
}

pub fn output_format_to_domain(value: i32) -> DomainOutputFormat {
    match pb::OutputFormat::try_from(value).unwrap_or(pb::OutputFormat::Unspecified) {
        pb::OutputFormat::SingleId => DomainOutputFormat::SingleId,
        pb::OutputFormat::FlexId => DomainOutputFormat::FlexId,
        pb::OutputFormat::Unspecified | pb::OutputFormat::RangeId => DomainOutputFormat::RangeId,
    }
}

pub fn json_to_prost_value(value: serde_json::Value) -> prost_types::Value {
    use prost_types::value::Kind;
    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(0),
        serde_json::Value::Bool(b) => Kind::BoolValue(b),
        serde_json::Value::Number(n) => Kind::NumberValue(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Kind::StringValue(s),
        serde_json::Value::Array(items) => Kind::ListValue(prost_types::ListValue {
            values: items.into_iter().map(json_to_prost_value).collect(),
        }),
        serde_json::Value::Object(map) => Kind::StructValue(prost_types::Struct {
            fields: map
                .into_iter()
                .map(|(k, v)| (k, json_to_prost_value(v)))
                .collect(),
        }),
    };
    prost_types::Value { kind: Some(kind) }
}

/// `google.protobuf.Value` は倍精度浮動小数点しか持たないため、`2^53` を超える整数は
/// この変換で精度が落ちる。
fn number_to_json(n: f64) -> serde_json::Value {
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        serde_json::Value::Number((n as i64).into())
    } else {
        serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    }
}

pub fn prost_value_to_json(value: Option<prost_types::Value>) -> serde_json::Value {
    use prost_types::value::Kind;
    match value.and_then(|v| v.kind) {
        None | Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(Kind::NumberValue(n)) => number_to_json(n),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(Kind::ListValue(list)) => serde_json::Value::Array(
            list.values
                .into_iter()
                .map(prost_value_to_json_owned)
                .collect(),
        ),
        Some(Kind::StructValue(s)) => serde_json::Value::Object(
            s.fields
                .into_iter()
                .map(|(k, v)| (k, prost_value_to_json_owned(v)))
                .collect(),
        ),
    }
}

fn prost_value_to_json_owned(value: prost_types::Value) -> serde_json::Value {
    prost_value_to_json(Some(value))
}

fn single_id_to_domain(id: pb::SingleId) -> RawSingleId {
    RawSingleId {
        z: id.z as u8,
        f: id.f,
        x: id.x,
        y: id.y,
        i: id.i,
        t: id.t,
    }
}

fn single_id_to_pb(id: RawSingleId) -> pb::SingleId {
    pb::SingleId {
        z: id.z as u32,
        f: id.f,
        x: id.x,
        y: id.y,
        i: id.i,
        t: id.t,
    }
}

fn range_id_to_domain(id: pb::RangeId) -> RawRangeId {
    RawRangeId {
        z: id.z as u8,
        f: id.f.map(|r| [r.min, r.max]),
        x: id.x.map(|r| [r.min, r.max]),
        y: id.y.map(|r| [r.min, r.max]),
        i: id.i,
        t: id.t.map(|r| [r.min, r.max]),
    }
}

fn range_id_to_pb(id: RawRangeId) -> pb::RangeId {
    pb::RangeId {
        z: id.z as u32,
        f: id.f.map(|[min, max]| pb::Int32Range { min, max }),
        x: id.x.map(|[min, max]| pb::Uint32Range { min, max }),
        y: id.y.map(|[min, max]| pb::Uint32Range { min, max }),
        i: id.i,
        t: id.t.map(|[min, max]| pb::Uint64Range { min, max }),
    }
}

fn flex_id_to_domain(id: pb::FlexId) -> RawFlexId {
    RawFlexId {
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

fn flex_id_to_pb(id: RawFlexId) -> pb::FlexId {
    pb::FlexId {
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

pub fn spatial_id_to_domain(id: pb::SpatialId) -> Result<DomainSpatialId, Status> {
    match id
        .kind
        .ok_or_else(|| Status::invalid_argument("spatial_id.kind must be set"))?
    {
        pb::spatial_id::Kind::SingleId(s) => Ok(DomainSpatialId::SingleId(single_id_to_domain(s))),
        pb::spatial_id::Kind::RangeId(r) => Ok(DomainSpatialId::RangeId(range_id_to_domain(r))),
        pb::spatial_id::Kind::FlexId(f) => Ok(DomainSpatialId::FlexId(flex_id_to_domain(f))),
    }
}

pub fn spatial_ids_to_domain(ids: Vec<pb::SpatialId>) -> Result<Vec<DomainSpatialId>, Status> {
    ids.into_iter().map(spatial_id_to_domain).collect()
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

pub fn get_data_response_to_pb(response: GetDataResponse) -> pb::SearchDataResponse {
    match response {
        GetDataResponse::Single(GetDataResponseSingle { dictionary, data }) => {
            pb::SearchDataResponse {
                dictionary: dictionary.into_iter().map(json_to_prost_value).collect(),
                data: data
                    .into_iter()
                    .map(|g| {
                        data_group_to_pb(g, |id| pb::SpatialId {
                            kind: Some(pb::spatial_id::Kind::SingleId(single_id_to_pb(id))),
                        })
                    })
                    .collect(),
            }
        }
        GetDataResponse::Range(GetDataResponseRange { dictionary, data }) => {
            pb::SearchDataResponse {
                dictionary: dictionary.into_iter().map(json_to_prost_value).collect(),
                data: data
                    .into_iter()
                    .map(|g| {
                        data_group_to_pb(g, |id| pb::SpatialId {
                            kind: Some(pb::spatial_id::Kind::RangeId(range_id_to_pb(id))),
                        })
                    })
                    .collect(),
            }
        }
        GetDataResponse::Flex(GetDataResponseFlex { dictionary, data }) => pb::SearchDataResponse {
            dictionary: dictionary.into_iter().map(json_to_prost_value).collect(),
            data: data
                .into_iter()
                .map(|g| {
                    data_group_to_pb(g, |id| pb::SpatialId {
                        kind: Some(pb::spatial_id::Kind::FlexId(flex_id_to_pb(id))),
                    })
                })
                .collect(),
        },
    }
}
