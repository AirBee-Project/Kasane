//! クエリ DSL (`QueryNode` とその周辺) の変換。レスポンス側は `SearchDataResponse` を
//! そのまま使う（[`super::convert_data`]）ため、ここは
//! リクエスト方向（proto → ドメイン）だけを持つ。

use tonic::Status;

use super::convert::enum_from_i32;
use super::pb;
use crate::models::ValueLiteral;
use crate::models::query::{
    Direction, ExecuteQueryRequest, FalloffPattern, FilterCondition, MappingEntry, MathOperand,
    MathOperator, MergePolicyKind, QueryNode,
};

fn required<T>(value: Option<T>, field: &str) -> Result<T, Status> {
    value.ok_or_else(|| Status::invalid_argument(format!("{field} must be set")))
}

impl TryFrom<pb::MergePolicyKind> for MergePolicyKind {
    type Error = Status;

    fn try_from(value: pb::MergePolicyKind) -> Result<Self, Self::Error> {
        match value {
            pb::MergePolicyKind::Overwrite => Ok(Self::Overwrite),
            pb::MergePolicyKind::KeepExisting => Ok(Self::KeepExisting),
            pb::MergePolicyKind::Sum => Ok(Self::Sum),
            pb::MergePolicyKind::Max => Ok(Self::Max),
            pb::MergePolicyKind::Min => Ok(Self::Min),
            pb::MergePolicyKind::Average => Ok(Self::Average),
            pb::MergePolicyKind::Difference => Ok(Self::Difference),
            pb::MergePolicyKind::Unspecified => {
                Err(Status::invalid_argument("policy must be specified"))
            }
        }
    }
}

impl TryFrom<i32> for MergePolicyKind {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        enum_from_i32(value, pb::MergePolicyKind::Unspecified).try_into()
    }
}

impl TryFrom<pb::MathOperator> for MathOperator {
    type Error = Status;

    fn try_from(value: pb::MathOperator) -> Result<Self, Self::Error> {
        match value {
            pb::MathOperator::Add => Ok(Self::Add),
            pb::MathOperator::Subtract => Ok(Self::Subtract),
            pb::MathOperator::Multiply => Ok(Self::Multiply),
            pb::MathOperator::Divide => Ok(Self::Divide),
            pb::MathOperator::Unspecified => {
                Err(Status::invalid_argument("operator must be specified"))
            }
        }
    }
}

impl TryFrom<i32> for MathOperator {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        enum_from_i32(value, pb::MathOperator::Unspecified).try_into()
    }
}

impl From<pb::FalloffPattern> for FalloffPattern {
    fn from(value: pb::FalloffPattern) -> Self {
        match value {
            pb::FalloffPattern::QuadraticIn => Self::QuadraticIn,
            pb::FalloffPattern::QuadraticOut => Self::QuadraticOut,
            pb::FalloffPattern::Unspecified | pb::FalloffPattern::Linear => Self::Linear,
        }
    }
}

impl From<i32> for FalloffPattern {
    fn from(value: i32) -> Self {
        enum_from_i32(value, pb::FalloffPattern::Unspecified).into()
    }
}

impl TryFrom<i32> for Direction {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use pb::Direction as P;
        match enum_from_i32(value, P::Unspecified) {
            P::Upper => Ok(Self::Upper),
            P::Lower => Ok(Self::Lower),
            P::Unspecified => Err(Status::invalid_argument("direction must be specified")),
        }
    }
}

impl TryFrom<pb::MathOperand> for MathOperand {
    type Error = Status;

    fn try_from(operand: pb::MathOperand) -> Result<Self, Self::Error> {
        use pb::math_operand::Value as P;
        match operand
            .value
            .ok_or_else(|| Status::invalid_argument("operand.value must be set"))?
        {
            P::IntValue(v) => Ok(Self::Int(v)),
            P::FloatValue(v) => Ok(Self::Float(v)),
        }
    }
}

impl TryFrom<pb::FilterCondition> for FilterCondition {
    type Error = Status;

    fn try_from(condition: pb::FilterCondition) -> Result<Self, Self::Error> {
        use pb::filter_condition::Mode;
        match required(condition.mode, "condition.mode")? {
            Mode::Equals(e) => Ok(Self::Equals {
                value: e.value.map(Into::into).unwrap_or(ValueLiteral::Null),
            }),
            Mode::InRange(r) => Ok(Self::InRange {
                min: r.min.map(Into::into),
                max: r.max.map(Into::into),
            }),
            Mode::NotInRange(r) => Ok(Self::NotInRange {
                min: r.min.map(Into::into),
                max: r.max.map(Into::into),
            }),
        }
    }
}

impl From<pb::MappingEntry> for MappingEntry {
    fn from(entry: pb::MappingEntry) -> Self {
        Self {
            from: entry.from.map(Into::into).unwrap_or(ValueLiteral::Null),
            to: entry.to.map(Into::into).unwrap_or(ValueLiteral::Null),
        }
    }
}

impl TryFrom<pb::QueryNode> for QueryNode {
    type Error = Status;

    fn try_from(node: pb::QueryNode) -> Result<Self, Self::Error> {
        use pb::query_node::Node;

        Ok(match required(node.node, "query.node")? {
            Node::Source(s) => Self::Source {
                database: s.database,
                table: s.table,
            },
            Node::FilterValues(f) => Self::FilterValues {
                input: Box::new(
                    required(f.input, "filter_values.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                condition: required(f.condition, "filter_values.condition")?.try_into()?,
            },
            Node::ShiftX(s) => Self::ShiftX {
                input: Box::new(
                    required(s.input, "shift_x.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: s.z as u8,
                index: s.index,
            },
            Node::ShiftY(s) => Self::ShiftY {
                input: Box::new(
                    required(s.input, "shift_y.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: s.z as u8,
                index: s.index,
            },
            Node::ShiftF(s) => Self::ShiftF {
                input: Box::new(
                    required(s.input, "shift_f.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: s.z as u8,
                index: s.index,
            },
            Node::ZoomOut(z0) => Self::ZoomOut {
                input: Box::new(
                    required(z0.input, "zoom_out.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: z0.z as u8,
                policy: z0.policy.try_into()?,
            },
            Node::ExtrudeX(e) => Self::ExtrudeX {
                input: Box::new(
                    required(e.input, "extrude_x.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: e.z as u8,
                start: e.start,
                end: e.end,
                policy: e.policy.try_into()?,
            },
            Node::ExtrudeY(e) => Self::ExtrudeY {
                input: Box::new(
                    required(e.input, "extrude_y.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: e.z as u8,
                start: e.start,
                end: e.end,
                policy: e.policy.try_into()?,
            },
            Node::ExtrudeF(e) => Self::ExtrudeF {
                input: Box::new(
                    required(e.input, "extrude_f.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: e.z as u8,
                start: e.start,
                end: e.end,
                policy: e.policy.try_into()?,
            },
            Node::FalloffX(f) => Self::FalloffX {
                input: Box::new(
                    required(f.input, "falloff_x.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: f.z as u8,
                radius: f.radius,
                pattern: f.pattern.into(),
                direction: f.direction.map(TryInto::try_into).transpose()?,
                policy: f.policy.try_into()?,
            },
            Node::FalloffY(f) => Self::FalloffY {
                input: Box::new(
                    required(f.input, "falloff_y.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: f.z as u8,
                radius: f.radius,
                pattern: f.pattern.into(),
                direction: f.direction.map(TryInto::try_into).transpose()?,
                policy: f.policy.try_into()?,
            },
            Node::FalloffF(f) => Self::FalloffF {
                input: Box::new(
                    required(f.input, "falloff_f.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                z: f.z as u8,
                radius: f.radius,
                pattern: f.pattern.into(),
                direction: f.direction.map(TryInto::try_into).transpose()?,
                policy: f.policy.try_into()?,
            },
            Node::Merge(m) => Self::Merge {
                left: Box::new(
                    required(m.left, "merge.left")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                right: Box::new(
                    required(m.right, "merge.right")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                default: m
                    .default_value
                    .map(Into::into)
                    .unwrap_or(ValueLiteral::Null),
                policy: m.policy.try_into()?,
            },
            Node::Difference(s) => Self::Difference {
                left: Box::new(
                    required(s.left, "difference.left")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                right: Box::new(
                    required(s.right, "difference.right")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
            },
            Node::Intersection(s) => Self::Intersection {
                left: Box::new(
                    required(s.left, "intersection.left")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                right: Box::new(
                    required(s.right, "intersection.right")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
            },
            Node::MapValues(m) => Self::MapValues {
                input: Box::new(
                    required(m.input, "map_values.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                output_type: m.output_type.try_into()?,
                mapping: m.mapping.into_iter().map(Into::into).collect(),
                default: m
                    .default_value
                    .map(Into::into)
                    .unwrap_or(ValueLiteral::Null),
            },
            Node::MathValues(m) => Self::MathValues {
                input: Box::new(
                    required(m.input, "math_values.input")?
                        .as_ref()
                        .clone()
                        .try_into()?,
                ),
                operator: m.operator.try_into()?,
                operand: required(m.operand, "math_values.operand")?.try_into()?,
            },
        })
    }
}

pub struct ExecuteQuery {
    pub request: ExecuteQueryRequest,
    pub format: crate::models::database::table::data::OutputFormat,
    pub limit: Option<usize>,
}

impl TryFrom<pb::ExecuteQueryRequest> for ExecuteQuery {
    type Error = Status;

    fn try_from(req: pb::ExecuteQueryRequest) -> Result<Self, Self::Error> {
        let value_type = req.value_type.map(TryInto::try_into).transpose()?;
        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let query = required(req.query, "query")?.try_into()?;

        Ok(Self {
            request: ExecuteQueryRequest {
                value_type,
                spatial_ids,
                query,
            },
            format: req.format.into(),
            limit: req.limit.map(|v| v as usize),
        })
    }
}
