//! テストがクエリ式や空間IDを組み立てるための、`pb::*` を直接組み立てるだけのビルダー関数群。

use kasane::grpc::pb;

pub fn table_data_type(name: &str) -> pb::TableDataType {
    match name {
        "Text" => pb::TableDataType::Text,
        "Int" => pb::TableDataType::Int,
        "Boolean" => pb::TableDataType::Boolean,
        "Enum" => pb::TableDataType::Enum,
        "Presence" => pb::TableDataType::Presence,
        other => panic!("unknown data_type: {other}"),
    }
}

pub fn single_id(z: u32, f: i32, x: u32, y: u32) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::SingleId(pb::SingleId {
            z,
            f,
            x,
            y,
            i: None,
            t: None,
        })),
    }
}

pub fn single_id_with_time(z: u32, f: i32, x: u32, y: u32, i: u64, t: u64) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::SingleId(pb::SingleId {
            z,
            f,
            x,
            y,
            i: Some(i),
            t: Some(t),
        })),
    }
}

pub fn range_id(
    z: u32,
    f: Option<(i32, i32)>,
    x: Option<(u32, u32)>,
    y: Option<(u32, u32)>,
) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::RangeId(pb::RangeId {
            z,
            f: f.map(|(min, max)| pb::Int32Range { min, max }),
            x: x.map(|(min, max)| pb::Uint32Range { min, max }),
            y: y.map(|(min, max)| pb::Uint32Range { min, max }),
            i: None,
            t: None,
        })),
    }
}

pub fn range_id_with_time(
    z: u32,
    f: Option<(i32, i32)>,
    x: Option<(u32, u32)>,
    y: Option<(u32, u32)>,
    i: u64,
    t: (u64, u64),
) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::RangeId(pb::RangeId {
            z,
            f: f.map(|(min, max)| pb::Int32Range { min, max }),
            x: x.map(|(min, max)| pb::Uint32Range { min, max }),
            y: y.map(|(min, max)| pb::Uint32Range { min, max }),
            i: Some(i),
            t: Some(pb::Uint64Range { min: t.0, max: t.1 }),
        })),
    }
}

pub fn flex_id(
    f_zoomlevel: u32,
    f_index: i32,
    x_zoomlevel: u32,
    x_index: u32,
    y_zoomlevel: u32,
    y_index: u32,
) -> pb::SpatialId {
    pb::SpatialId {
        kind: Some(pb::spatial_id::Kind::FlexId(pb::FlexId {
            f_zoomlevel,
            f_index,
            x_zoomlevel,
            x_index,
            y_zoomlevel,
            y_index,
            t_zoomlevel: None,
            t_index: None,
        })),
    }
}

pub fn num(v: f64) -> pb::TypedValue {
    pb::TypedValue {
        kind: Some(pb::typed_value::Kind::IntVal(v as i64)),
    }
}

pub fn text(v: &str) -> pb::TypedValue {
    pb::TypedValue {
        kind: Some(pb::typed_value::Kind::StringVal(v.to_string())),
    }
}

pub fn boolean(v: bool) -> pb::TypedValue {
    pb::TypedValue {
        kind: Some(pb::typed_value::Kind::BoolVal(v)),
    }
}

pub fn null_val() -> pb::TypedValue {
    pb::TypedValue {
        kind: Some(pb::typed_value::Kind::NullVal(0)),
    }
}

pub fn value_as_f64(v: &pb::TypedValue) -> Option<f64> {
    match &v.kind {
        Some(pb::typed_value::Kind::IntVal(n)) => Some(*n as f64),
        _ => None,
    }
}

pub fn value_as_str(v: &pb::TypedValue) -> Option<&str> {
    match &v.kind {
        Some(pb::typed_value::Kind::StringVal(s)) => Some(s.as_str()),
        _ => None,
    }
}

pub fn source(database: &str, table: &str) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::Source(pb::query_node::Source {
            database: database.to_string(),
            table: table.to_string(),
        })),
    }
}

pub fn shift_x(input: pb::QueryNode, z: u32, index: i32) -> pb::QueryNode {
    shift(pb::query_node::Node::ShiftX, input, z, index)
}
pub fn shift_y(input: pb::QueryNode, z: u32, index: i32) -> pb::QueryNode {
    shift(pb::query_node::Node::ShiftY, input, z, index)
}
pub fn shift_f(input: pb::QueryNode, z: u32, index: i32) -> pb::QueryNode {
    shift(pb::query_node::Node::ShiftF, input, z, index)
}

fn shift(
    variant: fn(Box<pb::query_node::Shift>) -> pb::query_node::Node,
    input: pb::QueryNode,
    z: u32,
    index: i32,
) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(variant(Box::new(pb::query_node::Shift {
            input: Some(Box::new(input)),
            z,
            index,
        }))),
    }
}

pub fn zoom_out(input: pb::QueryNode, z: u32, policy: pb::MergePolicyKind) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::ZoomOut(Box::new(
            pb::query_node::ZoomOut {
                input: Some(Box::new(input)),
                z,
                policy: policy as i32,
            },
        ))),
    }
}

pub fn extrude_x(
    input: pb::QueryNode,
    z: u32,
    start: u32,
    end: u32,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    extrude_xy(pb::query_node::Node::ExtrudeX, input, z, start, end, policy)
}
pub fn extrude_y(
    input: pb::QueryNode,
    z: u32,
    start: u32,
    end: u32,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    extrude_xy(pb::query_node::Node::ExtrudeY, input, z, start, end, policy)
}

fn extrude_xy(
    variant: fn(Box<pb::query_node::ExtrudeXy>) -> pb::query_node::Node,
    input: pb::QueryNode,
    z: u32,
    start: u32,
    end: u32,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(variant(Box::new(pb::query_node::ExtrudeXy {
            input: Some(Box::new(input)),
            z,
            start,
            end,
            policy: policy as i32,
        }))),
    }
}

pub fn extrude_f(
    input: pb::QueryNode,
    z: u32,
    start: i32,
    end: i32,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::ExtrudeF(Box::new(
            pb::query_node::ExtrudeF {
                input: Some(Box::new(input)),
                z,
                start,
                end,
                policy: policy as i32,
            },
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn falloff_x(
    input: pb::QueryNode,
    z: u32,
    radius: u32,
    pattern: pb::FalloffPattern,
    direction: Option<pb::Direction>,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    falloff(
        pb::query_node::Node::FalloffX,
        input,
        z,
        radius,
        pattern,
        direction,
        policy,
    )
}
#[allow(clippy::too_many_arguments)]
pub fn falloff_y(
    input: pb::QueryNode,
    z: u32,
    radius: u32,
    pattern: pb::FalloffPattern,
    direction: Option<pb::Direction>,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    falloff(
        pb::query_node::Node::FalloffY,
        input,
        z,
        radius,
        pattern,
        direction,
        policy,
    )
}
#[allow(clippy::too_many_arguments)]
pub fn falloff_f(
    input: pb::QueryNode,
    z: u32,
    radius: u32,
    pattern: pb::FalloffPattern,
    direction: Option<pb::Direction>,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    falloff(
        pb::query_node::Node::FalloffF,
        input,
        z,
        radius,
        pattern,
        direction,
        policy,
    )
}

#[allow(clippy::too_many_arguments)]
fn falloff(
    variant: fn(Box<pb::query_node::Falloff>) -> pb::query_node::Node,
    input: pb::QueryNode,
    z: u32,
    radius: u32,
    pattern: pb::FalloffPattern,
    direction: Option<pb::Direction>,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(variant(Box::new(pb::query_node::Falloff {
            input: Some(Box::new(input)),
            z,
            radius,
            pattern: pattern as i32,
            direction: direction.map(|d| d as i32),
            policy: policy as i32,
        }))),
    }
}

pub fn merge(
    left: pb::QueryNode,
    right: pb::QueryNode,
    default: pb::TypedValue,
    policy: pb::MergePolicyKind,
) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::Merge(Box::new(
            pb::query_node::Merge {
                left: Some(Box::new(left)),
                right: Some(Box::new(right)),
                default_value: Some(default),
                policy: policy as i32,
            },
        ))),
    }
}

pub fn difference(left: pb::QueryNode, right: pb::QueryNode) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::Difference(Box::new(
            pb::query_node::SetOp {
                left: Some(Box::new(left)),
                right: Some(Box::new(right)),
            },
        ))),
    }
}

pub fn intersection(left: pb::QueryNode, right: pb::QueryNode) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::Intersection(Box::new(
            pb::query_node::SetOp {
                left: Some(Box::new(left)),
                right: Some(Box::new(right)),
            },
        ))),
    }
}

pub fn filter_equals(input: pb::QueryNode, value: pb::TypedValue) -> pb::QueryNode {
    filter_values(
        input,
        pb::filter_condition::Mode::Equals(pb::filter_condition::Equals { value: Some(value) }),
    )
}

pub fn filter_in_range(
    input: pb::QueryNode,
    min: Option<pb::TypedValue>,
    max: Option<pb::TypedValue>,
) -> pb::QueryNode {
    filter_values(
        input,
        pb::filter_condition::Mode::InRange(pb::filter_condition::InRange { min, max }),
    )
}

pub fn filter_not_in_range(
    input: pb::QueryNode,
    min: Option<pb::TypedValue>,
    max: Option<pb::TypedValue>,
) -> pb::QueryNode {
    filter_values(
        input,
        pb::filter_condition::Mode::NotInRange(pb::filter_condition::NotInRange { min, max }),
    )
}

fn filter_values(input: pb::QueryNode, mode: pb::filter_condition::Mode) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::FilterValues(Box::new(
            pb::query_node::FilterValues {
                input: Some(Box::new(input)),
                condition: Some(pb::FilterCondition { mode: Some(mode) }),
            },
        ))),
    }
}

pub fn mapping_entry(from: pb::TypedValue, to: pb::TypedValue) -> pb::MappingEntry {
    pb::MappingEntry {
        from: Some(from),
        to: Some(to),
    }
}

pub fn map_values(
    input: pb::QueryNode,
    output_type: pb::TableDataType,
    mapping: Vec<pb::MappingEntry>,
    default: pb::TypedValue,
) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::MapValues(Box::new(
            pb::query_node::MapValues {
                input: Some(Box::new(input)),
                output_type: output_type as i32,
                mapping,
                default_value: Some(default),
            },
        ))),
    }
}

pub fn math_values(
    input: pb::QueryNode,
    operator: pb::MathOperator,
    operand: pb::MathOperand,
) -> pb::QueryNode {
    pb::QueryNode {
        node: Some(pb::query_node::Node::MathValues(Box::new(
            pb::query_node::MathValues {
                input: Some(Box::new(input)),
                operator: operator as i32,
                operand: Some(operand),
            },
        ))),
    }
}

pub fn int_operand(v: i64) -> pb::MathOperand {
    pb::MathOperand {
        value: Some(pb::math_operand::Value::IntValue(v)),
    }
}

pub fn float_operand(v: f64) -> pb::MathOperand {
    pb::MathOperand {
        value: Some(pb::math_operand::Value::FloatValue(v)),
    }
}
