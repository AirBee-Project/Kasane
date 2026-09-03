//! クエリ DSL (`QueryNode` とその周辺) の変換。レスポンス側は `SearchDataResponse` を
//! そのまま使う（[`super::convert_data::get_data_response_to_pb`]）ため、ここは
//! リクエスト方向（proto → ドメイン）だけを持つ。

use tonic::Status;

use super::convert_data::{prost_value_to_json, spatial_ids_to_domain};
use super::pb;
use crate::models::query::{
    Direction, ExecuteQueryRequest, FalloffPattern, FilterCondition, MappingEntry, MathOperand,
    MathOperator, MergePolicyKind, QueryNode,
};

use super::convert::table_data_type_to_domain;
use super::convert_data::output_format_to_domain;

fn required<T>(value: Option<T>, field: &str) -> Result<T, Status> {
    value.ok_or_else(|| Status::invalid_argument(format!("{field} must be set")))
}

fn merge_policy_to_domain(value: i32) -> Result<MergePolicyKind, Status> {
    use pb::MergePolicyKind as P;
    match P::try_from(value).unwrap_or(P::Unspecified) {
        P::Overwrite => Ok(MergePolicyKind::Overwrite),
        P::KeepExisting => Ok(MergePolicyKind::KeepExisting),
        P::Sum => Ok(MergePolicyKind::Sum),
        P::Max => Ok(MergePolicyKind::Max),
        P::Min => Ok(MergePolicyKind::Min),
        P::Average => Ok(MergePolicyKind::Average),
        P::Difference => Ok(MergePolicyKind::Difference),
        P::Unspecified => Err(Status::invalid_argument("policy must be specified")),
    }
}

fn math_operator_to_domain(value: i32) -> Result<MathOperator, Status> {
    use pb::MathOperator as P;
    match P::try_from(value).unwrap_or(P::Unspecified) {
        P::Add => Ok(MathOperator::Add),
        P::Subtract => Ok(MathOperator::Subtract),
        P::Multiply => Ok(MathOperator::Multiply),
        P::Divide => Ok(MathOperator::Divide),
        P::Unspecified => Err(Status::invalid_argument("operator must be specified")),
    }
}

fn falloff_pattern_to_domain(value: i32) -> FalloffPattern {
    use pb::FalloffPattern as P;
    match P::try_from(value).unwrap_or(P::Unspecified) {
        P::QuadraticIn => FalloffPattern::QuadraticIn,
        P::QuadraticOut => FalloffPattern::QuadraticOut,
        P::Unspecified | P::Linear => FalloffPattern::Linear,
    }
}

fn direction_to_domain(value: Option<i32>) -> Result<Option<Direction>, Status> {
    use pb::Direction as P;
    let Some(value) = value else { return Ok(None) };
    match P::try_from(value).unwrap_or(P::Unspecified) {
        P::Upper => Ok(Some(Direction::Upper)),
        P::Lower => Ok(Some(Direction::Lower)),
        P::Unspecified => Err(Status::invalid_argument("direction must be specified")),
    }
}

fn math_operand_to_domain(operand: Option<pb::MathOperand>) -> Result<MathOperand, Status> {
    use pb::math_operand::Value as P;
    match required(operand, "operand")?.value {
        Some(P::IntValue(v)) => Ok(MathOperand::Int(v)),
        Some(P::FloatValue(v)) => Ok(MathOperand::Float(v)),
        None => Err(Status::invalid_argument("operand.value must be set")),
    }
}

fn filter_condition_to_domain(condition: pb::FilterCondition) -> Result<FilterCondition, Status> {
    use pb::filter_condition::Mode;
    match required(condition.mode, "condition.mode")? {
        Mode::Equals(e) => Ok(FilterCondition::Equals {
            value: prost_value_to_json(e.value),
        }),
        Mode::InRange(r) => Ok(FilterCondition::InRange {
            min: r.min.map(|v| prost_value_to_json(Some(v))),
            max: r.max.map(|v| prost_value_to_json(Some(v))),
        }),
        Mode::NotInRange(r) => Ok(FilterCondition::NotInRange {
            min: r.min.map(|v| prost_value_to_json(Some(v))),
            max: r.max.map(|v| prost_value_to_json(Some(v))),
        }),
    }
}

fn mapping_entry_to_domain(entry: pb::MappingEntry) -> MappingEntry {
    MappingEntry {
        from: prost_value_to_json(entry.from),
        to: prost_value_to_json(entry.to),
    }
}

/// `QueryNode` は自身を再帰的に参照するため、変換も再帰関数になる。
pub fn query_node_to_domain(node: pb::QueryNode) -> Result<QueryNode, Status> {
    use pb::query_node::Node;

    Ok(match required(node.node, "query.node")? {
        Node::Source(s) => QueryNode::Source {
            database: s.database,
            table: s.table,
        },
        Node::FilterValues(f) => QueryNode::FilterValues {
            input: Box::new(query_node_to_domain(*required(
                f.input,
                "filter_values.input",
            )?)?),
            condition: filter_condition_to_domain(required(
                f.condition,
                "filter_values.condition",
            )?)?,
        },
        Node::ShiftX(s) => QueryNode::ShiftX {
            input: Box::new(query_node_to_domain(*required(s.input, "shift_x.input")?)?),
            z: s.z as u8,
            index: s.index,
        },
        Node::ShiftY(s) => QueryNode::ShiftY {
            input: Box::new(query_node_to_domain(*required(s.input, "shift_y.input")?)?),
            z: s.z as u8,
            index: s.index,
        },
        Node::ShiftF(s) => QueryNode::ShiftF {
            input: Box::new(query_node_to_domain(*required(s.input, "shift_f.input")?)?),
            z: s.z as u8,
            index: s.index,
        },
        Node::ZoomOut(z0) => QueryNode::ZoomOut {
            input: Box::new(query_node_to_domain(*required(
                z0.input,
                "zoom_out.input",
            )?)?),
            z: z0.z as u8,
            policy: merge_policy_to_domain(z0.policy)?,
        },
        Node::ExtrudeX(e) => QueryNode::ExtrudeX {
            input: Box::new(query_node_to_domain(*required(
                e.input,
                "extrude_x.input",
            )?)?),
            z: e.z as u8,
            start: e.start,
            end: e.end,
            policy: merge_policy_to_domain(e.policy)?,
        },
        Node::ExtrudeY(e) => QueryNode::ExtrudeY {
            input: Box::new(query_node_to_domain(*required(
                e.input,
                "extrude_y.input",
            )?)?),
            z: e.z as u8,
            start: e.start,
            end: e.end,
            policy: merge_policy_to_domain(e.policy)?,
        },
        Node::ExtrudeF(e) => QueryNode::ExtrudeF {
            input: Box::new(query_node_to_domain(*required(
                e.input,
                "extrude_f.input",
            )?)?),
            z: e.z as u8,
            start: e.start,
            end: e.end,
            policy: merge_policy_to_domain(e.policy)?,
        },
        Node::FalloffX(f) => QueryNode::FalloffX {
            input: Box::new(query_node_to_domain(*required(
                f.input,
                "falloff_x.input",
            )?)?),
            z: f.z as u8,
            radius: f.radius,
            pattern: falloff_pattern_to_domain(f.pattern),
            direction: direction_to_domain(f.direction)?,
            policy: merge_policy_to_domain(f.policy)?,
        },
        Node::FalloffY(f) => QueryNode::FalloffY {
            input: Box::new(query_node_to_domain(*required(
                f.input,
                "falloff_y.input",
            )?)?),
            z: f.z as u8,
            radius: f.radius,
            pattern: falloff_pattern_to_domain(f.pattern),
            direction: direction_to_domain(f.direction)?,
            policy: merge_policy_to_domain(f.policy)?,
        },
        Node::FalloffF(f) => QueryNode::FalloffF {
            input: Box::new(query_node_to_domain(*required(
                f.input,
                "falloff_f.input",
            )?)?),
            z: f.z as u8,
            radius: f.radius,
            pattern: falloff_pattern_to_domain(f.pattern),
            direction: direction_to_domain(f.direction)?,
            policy: merge_policy_to_domain(f.policy)?,
        },
        Node::Merge(m) => QueryNode::Merge {
            left: Box::new(query_node_to_domain(*required(m.left, "merge.left")?)?),
            right: Box::new(query_node_to_domain(*required(m.right, "merge.right")?)?),
            default: prost_value_to_json(m.default_value),
            policy: merge_policy_to_domain(m.policy)?,
        },
        Node::Difference(s) => QueryNode::Difference {
            left: Box::new(query_node_to_domain(*required(s.left, "difference.left")?)?),
            right: Box::new(query_node_to_domain(*required(
                s.right,
                "difference.right",
            )?)?),
        },
        Node::Intersection(s) => QueryNode::Intersection {
            left: Box::new(query_node_to_domain(*required(
                s.left,
                "intersection.left",
            )?)?),
            right: Box::new(query_node_to_domain(*required(
                s.right,
                "intersection.right",
            )?)?),
        },
        Node::MapValues(m) => QueryNode::MapValues {
            input: Box::new(query_node_to_domain(*required(
                m.input,
                "map_values.input",
            )?)?),
            output_type: table_data_type_to_domain(m.output_type)?,
            mapping: m.mapping.into_iter().map(mapping_entry_to_domain).collect(),
            default: prost_value_to_json(m.default_value),
        },
        Node::MathValues(m) => QueryNode::MathValues {
            input: Box::new(query_node_to_domain(*required(
                m.input,
                "math_values.input",
            )?)?),
            operator: math_operator_to_domain(m.operator)?,
            operand: math_operand_to_domain(m.operand)?,
        },
    })
}

pub struct ExecuteQuery {
    pub request: ExecuteQueryRequest,
    pub format: crate::models::database::table::data::OutputFormat,
    pub limit: Option<usize>,
}

pub fn execute_query_request_to_domain(
    req: pb::ExecuteQueryRequest,
) -> Result<ExecuteQuery, Status> {
    let value_type = req.value_type.map(table_data_type_to_domain).transpose()?;
    let spatial_ids = spatial_ids_to_domain(req.spatial_ids)?;
    let query = query_node_to_domain(required(req.query, "query")?)?;

    Ok(ExecuteQuery {
        request: ExecuteQueryRequest {
            value_type,
            spatial_ids,
            query,
        },
        format: output_format_to_domain(req.format),
        limit: req.limit.map(|v| v as usize),
    })
}
