use crate::models::spatial_id::SpatialId;
use serde::Deserialize;
use utoipa::ToSchema;

/// 同一空間に複数の値が集まったときの集約規則。
#[derive(Debug, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MergePolicyKind {
    /// 後の値で上書きする
    Overwrite,
    /// 既存の値を優先する
    KeepExisting,
    /// 合計する
    Sum,
    /// 大きい方を採る
    Max,
    /// 小さい方を採る
    Min,
    /// 平均を採る
    Average,
    /// 差を採る
    Difference,
}

/// 値フィルタの条件。
#[derive(Debug, Deserialize, ToSchema, Clone)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum FilterCondition {
    /// `value` に一致する値だけを残す
    Equals {
        #[schema(example = json!("some_value"))]
        value: serde_json::Value,
    },
    /// 値が `min..=max`に入る値だけを残す
    InRange {
        #[serde(default)]
        #[schema(example = json!(0))]
        min: Option<serde_json::Value>,
        #[serde(default)]
        #[schema(example = json!(100))]
        max: Option<serde_json::Value>,
    },
    /// 値が `min..=max`に入る値を取り除く
    NotInRange {
        #[serde(default)]
        #[schema(example = json!(0))]
        min: Option<serde_json::Value>,
        #[serde(default)]
        #[schema(example = json!(100))]
        max: Option<serde_json::Value>,
    },
}

/// 対応表エントリ
#[derive(Debug, Deserialize, ToSchema, Clone)]
pub struct MappingEntry {
    /// 変換前の値
    #[schema(example = json!(1))]
    pub from: serde_json::Value,
    /// 変換後の値
    #[schema(example = json!("one"))]
    pub to: serde_json::Value,
}

/// 計算用のオペランド（整数と小数の両方を受け付ける）
#[derive(Debug, Deserialize, ToSchema, Clone, Copy, PartialEq)]
#[serde(untagged)]
pub enum MathOperand {
    Int(i64),
    Float(f64),
}

/// 四則演算の演算子
#[derive(Debug, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MathOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Deserialize, ToSchema, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QueryNode {
    /// 演算の起点。読み出し対象のテーブルを指定する。
    Source {
        #[schema(example = "example_database")]
        database: String,
        #[schema(example = "example_table")]
        table: String,
    },

    FilterValues {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[serde(flatten)]
        condition: FilterCondition,
    },

    /// X方向へ平行移動する
    ShiftX {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 1)]
        index: i32,
    },
    /// Y方向へ平行移動する
    ShiftY {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 1)]
        index: i32,
    },
    /// F(高度)方向へ平行移動する
    ShiftF {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 1)]
        index: i32,
    },

    /// 指定された値までズームレベルを落とし、`policy` で集約する
    ZoomOut {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 24)]
        z: u8,
        #[schema(example = "average")]
        policy: MergePolicyKind,
    },

    /// X方向の絶対座標範囲へ引き延ばす
    ExtrudeX {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 7300000)]
        start: u32,
        #[schema(example = 7300063)]
        end: u32,
        #[schema(example = "overwrite")]
        policy: MergePolicyKind,
    },
    /// Y方向の絶対座標範囲へ引き延ばす
    ExtrudeY {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 3276800)]
        start: u32,
        #[schema(example = 3276863)]
        end: u32,
        #[schema(example = "overwrite")]
        policy: MergePolicyKind,
    },
    /// F方向の絶対座標範囲へ引き延ばす
    ExtrudeF {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = -5)]
        start: i32,
        #[schema(example = 5)]
        end: i32,
        #[schema(example = "overwrite")]
        policy: MergePolicyKind,
    },

    /// X方向へ、指定距離で0になるよう値を線形減衰させる
    FalloffLinearX {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 3)]
        radius: u32,
        #[schema(example = "max")]
        policy: MergePolicyKind,
    },
    /// Y方向へ、指定距離で0になるよう値を線形減衰させる
    FalloffLinearY {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 3)]
        radius: u32,
        #[schema(example = "max")]
        policy: MergePolicyKind,
    },
    /// F方向へ、指定距離で0になるよう値を線形減衰させる
    FalloffLinearF {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = 25)]
        z: u8,
        #[schema(example = 3)]
        radius: u32,
        #[schema(example = "max")]
        policy: MergePolicyKind,
    },

    /// 2つの部分式を `policy` で重ね合わせる。
    /// 片側にしか値が無いセルは `default` を相手側の値とみなす。
    Merge {
        #[schema(no_recursion)]
        left: Box<QueryNode>,
        #[schema(no_recursion)]
        right: Box<QueryNode>,
        #[schema(example = json!(0))]
        default: serde_json::Value,
        #[schema(example = "overwrite")]
        policy: MergePolicyKind,
    },

    /// 値を対応表に基づいて変換する。対応表にない値は `default` になる。
    MapValues {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        /// この MapValues が出力する型。対応表の `to` や `default` はこの型として解釈される。
        #[schema(example = "Int")]
        output_type: crate::models::database::table::TableDataType,
        /// 変換前後の対応表。`from` の重複は 400 で拒否される。
        mapping: Vec<MappingEntry>,
        /// 対応表に存在しない値に使う既定値
        #[schema(example = json!("unknown"))]
        default: serde_json::Value,
    },

    /// 値に対して四則演算を行う
    MathValues {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        #[schema(example = "multiply")]
        operator: MathOperator,
        #[schema(value_type = f64, example = 1.5)]
        operand: MathOperand,
    },
}

impl QueryNode {
    /// このノードが持つ子部分式を列挙する。
    pub fn children(&self) -> impl Iterator<Item = &QueryNode> {
        let (a, b) = match self {
            QueryNode::Source { .. } => (None, None),
            QueryNode::FilterValues { input, .. }
            | QueryNode::ShiftX { input, .. }
            | QueryNode::ShiftY { input, .. }
            | QueryNode::ShiftF { input, .. }
            | QueryNode::ZoomOut { input, .. }
            | QueryNode::ExtrudeX { input, .. }
            | QueryNode::ExtrudeY { input, .. }
            | QueryNode::ExtrudeF { input, .. }
            | QueryNode::FalloffLinearX { input, .. }
            | QueryNode::FalloffLinearY { input, .. }
            | QueryNode::FalloffLinearF { input, .. }
            | QueryNode::MathValues { input, .. }
            | QueryNode::MapValues { input, .. } => (Some(&**input), None),
            QueryNode::Merge { left, right, .. } => (Some(&**left), Some(&**right)),
        };
        a.into_iter().chain(b)
    }

    /// このノードを根とする部分木を、根から順に列挙する（行きがけ順）。
    pub fn iter(&self) -> impl Iterator<Item = &QueryNode> {
        let mut stack = vec![self];
        core::iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children());
            Some(node)
        })
    }

    /// AST が参照する全ての `(database, table)` を、重複ありで集める。
    pub fn sources(&self) -> Vec<(&str, &str)> {
        self.iter()
            .filter_map(|node| match node {
                QueryNode::Source {
                    database, table, ..
                } => Some((database.as_str(), table.as_str())),
                _ => None,
            })
            .collect()
    }
}

/// Tableに対するQueryを表現する型
#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "value_type": "Int",
    "spatial_ids": [
        { "type": "rangeId", "z": 23, "f": [0, 0], "x": [7300000, 7300063], "y": [3276800, 3276863] }
    ],
    "query": {
        "type": "zoomOut",
        "z": 23,
        "policy": "average",
        "input": {
            "type": "falloffLinearY",
            "z": 25,
            "radius": 3,
            "policy": "max",
            "input": {
                "type": "falloffLinearX",
                "z": 25,
                "radius": 3,
                "policy": "max",
                 "input": {
                        "type": "source",
                        "database": "sensors",
                        "table": "heat_sources"
                }
            }
        }
    }
}))]
pub struct ExecuteQueryRequest {
    #[serde(default)]
    pub value_type: Option<crate::models::database::table::TableDataType>,
    pub spatial_ids: Vec<SpatialId>,
    pub query: QueryNode,
}
