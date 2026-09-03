use crate::models::spatial_id::SpatialId;
use serde::Deserialize;

/// 同一空間に複数の値が集まったときの集約規則。
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum FilterCondition {
    /// `value` に一致する値だけを残す
    Equals { value: serde_json::Value },
    /// 値が `min..=max`に入る値だけを残す
    InRange {
        #[serde(default)]
        min: Option<serde_json::Value>,
        #[serde(default)]
        max: Option<serde_json::Value>,
    },
    /// 値が `min..=max`に入る値を取り除く
    NotInRange {
        #[serde(default)]
        min: Option<serde_json::Value>,
        #[serde(default)]
        max: Option<serde_json::Value>,
    },
}

/// 対応表エントリ
#[derive(Debug, Deserialize, Clone)]
pub struct MappingEntry {
    /// 変換前の値
    pub from: serde_json::Value,
    /// 変換後の値
    pub to: serde_json::Value,
}

/// 計算用のオペランド（整数と小数の両方を受け付ける）
#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(untagged)]
pub enum MathOperand {
    Int(i64),
    Float(f64),
}

/// 四則演算の演算子
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MathOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum FalloffPattern {
    #[default]
    Linear,
    QuadraticIn,
    QuadraticOut,
}

impl From<FalloffPattern>
    for kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern
{
    fn from(val: FalloffPattern) -> Self {
        match val {
            FalloffPattern::Linear => Self::Linear,
            FalloffPattern::QuadraticIn => Self::QuadraticIn,
            FalloffPattern::QuadraticOut => Self::QuadraticOut,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Upper,
    Lower,
}

impl From<Direction> for kasane_logic::spatial_id::helpers::Side {
    fn from(val: Direction) -> Self {
        match val {
            Direction::Upper => Self::Upper,
            Direction::Lower => Self::Lower,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QueryNode {
    /// 演算の起点。読み出し対象のテーブルを指定する。
    Source { database: String, table: String },

    FilterValues {
        input: Box<Self>,
        #[serde(flatten)]
        condition: FilterCondition,
    },

    /// X方向へ平行移動する
    ShiftX { input: Box<Self>, z: u8, index: i32 },
    /// Y方向へ平行移動する
    ShiftY { input: Box<Self>, z: u8, index: i32 },
    /// F(高度)方向へ平行移動する
    ShiftF { input: Box<Self>, z: u8, index: i32 },

    /// 指定された値までズームレベルを落とし、`policy` で集約する
    ZoomOut {
        input: Box<Self>,
        z: u8,
        policy: MergePolicyKind,
    },

    /// X方向の絶対座標範囲へ引き延ばす
    ExtrudeX {
        input: Box<Self>,
        z: u8,
        start: u32,
        end: u32,
        policy: MergePolicyKind,
    },
    /// Y方向の絶対座標範囲へ引き延ばす
    ExtrudeY {
        input: Box<Self>,
        z: u8,
        start: u32,
        end: u32,
        policy: MergePolicyKind,
    },
    /// F方向の絶対座標範囲へ引き延ばす
    ExtrudeF {
        input: Box<Self>,
        z: u8,
        start: i32,
        end: i32,
        policy: MergePolicyKind,
    },

    /// X方向へ、指定距離で0になるよう値を減衰させる
    FalloffX {
        input: Box<Self>,
        z: u8,
        radius: u32,
        #[serde(default)]
        pattern: FalloffPattern,
        #[serde(default)]
        direction: Option<Direction>,
        policy: MergePolicyKind,
    },
    /// Y方向へ、指定距離で0になるよう値を減衰させる
    FalloffY {
        input: Box<Self>,
        z: u8,
        radius: u32,
        #[serde(default)]
        pattern: FalloffPattern,
        #[serde(default)]
        direction: Option<Direction>,
        policy: MergePolicyKind,
    },
    /// F方向へ、指定距離で0になるよう値を減衰させる
    FalloffF {
        input: Box<Self>,
        z: u8,
        radius: u32,
        #[serde(default)]
        pattern: FalloffPattern,
        #[serde(default)]
        direction: Option<Direction>,
        policy: MergePolicyKind,
    },

    /// 2つの部分式を `policy` で重ね合わせる。
    /// 片側にしか値が無い FlexId は `default` を相手側の値とみなす。
    Merge {
        left: Box<Self>,
        right: Box<Self>,
        default: serde_json::Value,
        policy: MergePolicyKind,
    },

    /// 左側の結果から、右側の空間と重なる部分を取り除く。
    Difference { left: Box<Self>, right: Box<Self> },

    /// 左右の空間が重なる部分だけを残す。値は左側のものが維持される。
    Intersection { left: Box<Self>, right: Box<Self> },

    /// 値を対応表に基づいて変換する。対応表にない値は `default` になる。
    MapValues {
        input: Box<Self>,
        /// この MapValues が出力する型。対応表の `to` や `default` はこの型として解釈される。
        output_type: crate::models::database::table::TableDataType,
        /// 変換前後の対応表。`from` の重複は 400 で拒否される。
        mapping: Vec<MappingEntry>,
        /// 対応表に存在しない値に使う既定値
        default: serde_json::Value,
    },

    /// 値に対して四則演算を行う
    MathValues {
        input: Box<Self>,
        operator: MathOperator,
        operand: MathOperand,
    },
}

impl QueryNode {
    /// このノードが持つ子部分式を列挙する。
    pub fn children(&self) -> impl Iterator<Item = &Self> {
        let (a, b) = match self {
            Self::Source { .. } => (None, None),
            Self::FilterValues { input, .. }
            | Self::ShiftX { input, .. }
            | Self::ShiftY { input, .. }
            | Self::ShiftF { input, .. }
            | Self::ZoomOut { input, .. }
            | Self::ExtrudeX { input, .. }
            | Self::ExtrudeY { input, .. }
            | Self::ExtrudeF { input, .. }
            | Self::FalloffX { input, .. }
            | Self::FalloffY { input, .. }
            | Self::FalloffF { input, .. }
            | Self::MathValues { input, .. }
            | Self::MapValues { input, .. } => (Some(&**input), None),
            Self::Merge { left, right, .. }
            | Self::Difference { left, right }
            | Self::Intersection { left, right } => (Some(&**left), Some(&**right)),
        };
        a.into_iter().chain(b)
    }

    /// このノードを根とする部分木を、根から順に列挙する（行きがけ順）。
    pub fn iter(&self) -> impl Iterator<Item = &Self> {
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
                Self::Source {
                    database, table, ..
                } => Some((database.as_str(), table.as_str())),
                _ => None,
            })
            .collect()
    }
}

/// Tableに対するQueryを表現する型
#[derive(Debug, Deserialize)]
pub struct ExecuteQueryRequest {
    #[serde(default)]
    pub value_type: Option<crate::models::database::table::TableDataType>,
    pub spatial_ids: Vec<SpatialId>,
    pub query: QueryNode,
}
