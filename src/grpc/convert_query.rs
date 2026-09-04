//! Query関連の変換
use tonic::Status;

use super::convert::{enum_from_i32, required, u8_from_u32};
use super::pb;
use crate::error::AppError;
use crate::models::query::{
    Direction, ExecuteQueryRequest, FalloffPattern, FilterCondition, MappingEntry, MathOperand,
    MathOperator, MergePolicyKind, QueryNode,
};

fn required_node(node: Option<Box<pb::QueryNode>>, field: &str) -> Result<Box<QueryNode>, Status> {
    let boxed = required(node, field)?;
    Ok(Box::new((*boxed).try_into()?))
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
            pb::MergePolicyKind::Unspecified => Err(AppError::InvalidArgument {
                reason: "policy must be specified".to_string(),
            }
            .into()),
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
            pb::MathOperator::Unspecified => Err(AppError::InvalidArgument {
                reason: "operator must be specified".to_string(),
            }
            .into()),
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
            P::Unspecified => Err(AppError::InvalidArgument {
                reason: "direction must be specified".to_string(),
            }
            .into()),
        }
    }
}

impl TryFrom<pb::MathOperand> for MathOperand {
    type Error = Status;

    fn try_from(operand: pb::MathOperand) -> Result<Self, Self::Error> {
        use pb::math_operand::Value as P;
        match required(operand.value, "operand.value")? {
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
                value: required(e.value, "condition.equals.value")?.into(),
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
            from: entry.from.map(Into::into).unwrap_or_default(),
            to: entry.to.map(Into::into).unwrap_or_default(),
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
                input: required_node(f.input, "filter_values.input")?,
                condition: required(f.condition, "filter_values.condition")?.try_into()?,
            },
            Node::ShiftX(s) => Self::ShiftX {
                input: required_node(s.input, "shift_x.input")?,
                z: u8_from_u32(s.z, "shift_x.z")?,
                index: s.index,
            },
            Node::ShiftY(s) => Self::ShiftY {
                input: required_node(s.input, "shift_y.input")?,
                z: u8_from_u32(s.z, "shift_y.z")?,
                index: s.index,
            },
            Node::ShiftF(s) => Self::ShiftF {
                input: required_node(s.input, "shift_f.input")?,
                z: u8_from_u32(s.z, "shift_f.z")?,
                index: s.index,
            },
            Node::ZoomOut(z0) => Self::ZoomOut {
                input: required_node(z0.input, "zoom_out.input")?,
                z: u8_from_u32(z0.z, "zoom_out.z")?,
                policy: z0.policy.try_into()?,
            },
            Node::ExtrudeX(e) => Self::ExtrudeX {
                input: required_node(e.input, "extrude_x.input")?,
                z: u8_from_u32(e.z, "extrude_x.z")?,
                start: e.start,
                end: e.end,
                policy: e.policy.try_into()?,
            },
            Node::ExtrudeY(e) => Self::ExtrudeY {
                input: required_node(e.input, "extrude_y.input")?,
                z: u8_from_u32(e.z, "extrude_y.z")?,
                start: e.start,
                end: e.end,
                policy: e.policy.try_into()?,
            },
            Node::ExtrudeF(e) => Self::ExtrudeF {
                input: required_node(e.input, "extrude_f.input")?,
                z: u8_from_u32(e.z, "extrude_f.z")?,
                start: e.start,
                end: e.end,
                policy: e.policy.try_into()?,
            },
            Node::FalloffX(f) => Self::FalloffX {
                input: required_node(f.input, "falloff_x.input")?,
                z: u8_from_u32(f.z, "falloff_x.z")?,
                radius: f.radius,
                pattern: f.pattern.into(),
                direction: f.direction.map(TryInto::try_into).transpose()?,
                policy: f.policy.try_into()?,
            },
            Node::FalloffY(f) => Self::FalloffY {
                input: required_node(f.input, "falloff_y.input")?,
                z: u8_from_u32(f.z, "falloff_y.z")?,
                radius: f.radius,
                pattern: f.pattern.into(),
                direction: f.direction.map(TryInto::try_into).transpose()?,
                policy: f.policy.try_into()?,
            },
            Node::FalloffF(f) => Self::FalloffF {
                input: required_node(f.input, "falloff_f.input")?,
                z: u8_from_u32(f.z, "falloff_f.z")?,
                radius: f.radius,
                pattern: f.pattern.into(),
                direction: f.direction.map(TryInto::try_into).transpose()?,
                policy: f.policy.try_into()?,
            },
            Node::Merge(m) => Self::Merge {
                left: required_node(m.left, "merge.left")?,
                right: required_node(m.right, "merge.right")?,
                default: m.default_value.map(Into::into).unwrap_or_default(),
                policy: m.policy.try_into()?,
            },
            Node::Difference(s) => Self::Difference {
                left: required_node(s.left, "difference.left")?,
                right: required_node(s.right, "difference.right")?,
            },
            Node::Intersection(s) => Self::Intersection {
                left: required_node(s.left, "intersection.left")?,
                right: required_node(s.right, "intersection.right")?,
            },
            Node::MapValues(m) => Self::MapValues {
                input: required_node(m.input, "map_values.input")?,
                output_type: m.output_type.try_into()?,
                mapping: m.mapping.into_iter().map(Into::into).collect(),
                default: m.default_value.map(Into::into).unwrap_or_default(),
            },
            Node::MathValues(m) => Self::MathValues {
                input: required_node(m.input, "math_values.input")?,
                operator: m.operator.try_into()?,
                operand: required(m.operand, "math_values.operand")?.try_into()?,
            },
        })
    }
}

pub struct ExecuteQuery {
    pub request: ExecuteQueryRequest,
    pub format: crate::models::database::table::data::OutputFormat,
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
        })
    }
}
