//! `/query` エンドポイントの入口となるクエリ DSL。
//!
//! OpenAPI に露出する形はこちら（Kasane）が定義し、AST の最適化・実行は Kasane-Logic に委ねる。
//! Kasane-Logic 側にしか存在しない演算子は、ここにマッピングを追加しない限り API には現れない。

use crate::models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId};
use serde::Deserialize;
use utoipa::ToSchema;

/// 同一空間に複数の値が集まったときの集約規則。
///
/// Kasane-Logic の `merge_policy` の各型に対応する。
///
/// - `overwrite`: 後の値で上書きする
/// - `keepExisting`: 既存の値を優先する
/// - `sum`: 合計する
/// - `max`: 大きい方を採る
/// - `min`: 小さい方を採る
/// - `average`: 平均を採る
/// - `difference`: 差を採る
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

/// 値フィルタの条件種別。
#[derive(Debug, Deserialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FilterMode {
    /// `value` に一致するセルだけを残す
    Equals,
    /// 値が `min..=max`（閉区間）に入るセルだけを残す
    InRange,
    /// 値が `min..=max`（閉区間）に入るセルを取り除く
    NotInRange,
}

/// 格納値からクエリの値型への変換表。
///
/// `from` はそのテーブルの `data_type` の値、`to` はクエリの値型の値。
#[derive(Debug, Deserialize, ToSchema, Clone)]
pub struct ValueConvert {
    pub entries: Vec<ValueConvertEntry>,
    /// 変換表に載っていない値の扱い。省略するとそのセルを結果から除外する。
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, ToSchema, Clone)]
pub struct ValueConvertEntry {
    pub from: serde_json::Value,
    pub to: serde_json::Value,
}

/// クエリ式のノード（AST）。
///
/// `source` を葉とする木構造。`merge` は2つの部分式を持つため、複数テーブルを
/// 1つのクエリで扱える。参照する全データベースに Read 権限が必要。
#[derive(Debug, Deserialize, ToSchema, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QueryNode {
    /// 演算の起点。読み出し対象のテーブルを指定する。
    Source {
        #[schema(example = "example_database")]
        database: String,
        #[schema(example = "example_table")]
        table: String,
        /// テーブルの格納値をクエリの値型へ写す変換表（任意）。
        ///
        /// 省略した場合、このテーブルの `data_type` はクエリの値型と一致していなければならない。
        /// 指定すると、型の異なるテーブル同士を1つのクエリで扱える。
        convert: Option<ValueConvert>,
    },

    /// 値の条件でセルを絞り込む。空間的な形は変えない。
    FilterValues {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        mode: FilterMode,
        /// `equals` では対象の値、範囲指定では下限（閉区間・省略で無制限）
        #[serde(default)]
        value: Option<serde_json::Value>,
        #[serde(default)]
        min: Option<serde_json::Value>,
        #[serde(default)]
        max: Option<serde_json::Value>,
    },

    /// X方向へ平行移動する
    ShiftX {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        index: i32,
    },
    /// Y方向へ平行移動する
    ShiftY {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        index: i32,
    },
    /// F(高度)方向へ平行移動する
    ShiftF {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        index: i32,
    },

    /// 指定ズームレベルまで解像度を落とし、集まった子を `policy` で集約する
    ZoomOut {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        policy: MergePolicyKind,
    },

    /// X方向の絶対座標範囲へ引き延ばす
    ExtrudeX {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        start: u32,
        end: u32,
        policy: MergePolicyKind,
    },
    /// Y方向の絶対座標範囲へ引き延ばす
    ExtrudeY {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        start: u32,
        end: u32,
        policy: MergePolicyKind,
    },
    /// F方向の絶対座標範囲へ引き延ばす
    ExtrudeF {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        start: i32,
        end: i32,
        policy: MergePolicyKind,
    },

    /// X方向へ、指定距離で0になるよう値を線形減衰させる
    FalloffLinearX {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        radius: u32,
        policy: MergePolicyKind,
    },
    /// Y方向へ、指定距離で0になるよう値を線形減衰させる
    FalloffLinearY {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        radius: u32,
        policy: MergePolicyKind,
    },
    /// F方向へ、指定距離で0になるよう値を線形減衰させる
    FalloffLinearF {
        #[schema(no_recursion)]
        input: Box<QueryNode>,
        z: u8,
        radius: u32,
        policy: MergePolicyKind,
    },

    /// 2つの部分式を `policy` で重ね合わせる。
    /// 片側にしか値が無いセルは `default` を相手側の値とみなす。
    Merge {
        #[schema(no_recursion)]
        left: Box<QueryNode>,
        #[schema(no_recursion)]
        right: Box<QueryNode>,
        default: serde_json::Value,
        policy: MergePolicyKind,
    },
}

impl QueryNode {
    /// AST が参照する全ての `(database, table)` を、出現順・重複ありで集める。
    pub fn sources(&self) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        self.collect_sources(&mut out);
        out
    }

    fn collect_sources<'a>(&'a self, out: &mut Vec<(&'a str, &'a str)>) {
        match self {
            QueryNode::Source {
                database, table, ..
            } => out.push((database, table)),
            QueryNode::FilterValues { input, .. } => input.collect_sources(out),
            QueryNode::ShiftX { input, .. }
            | QueryNode::ShiftY { input, .. }
            | QueryNode::ShiftF { input, .. }
            | QueryNode::ZoomOut { input, .. }
            | QueryNode::ExtrudeX { input, .. }
            | QueryNode::ExtrudeY { input, .. }
            | QueryNode::ExtrudeF { input, .. }
            | QueryNode::FalloffLinearX { input, .. }
            | QueryNode::FalloffLinearY { input, .. }
            | QueryNode::FalloffLinearF { input, .. } => input.collect_sources(out),
            QueryNode::Merge { left, right, .. } => {
                left.collect_sources(out);
                right.collect_sources(out);
            }
        }
    }
}

/// `POST /query` のリクエストボディ。
///
/// `search` と同じ空間ID指定に、実行するクエリ式（`query`）を足した形。
#[derive(Debug, Deserialize, ToSchema)]
pub struct ExecuteQueryRequest {
    /// クエリ結果の値型。
    ///
    /// 省略時は、変換表を持たない全ソースの `data_type` が一致していればそれを採用する。
    /// 変換表を使う場合は明示が必要。
    #[serde(default)]
    pub value_type: Option<crate::models::database::table::TableDataType>,
    /// 結果を取得したい空間IDの配列
    pub spatial_ids: Vec<SpatialId>,
    #[serde(default)]
    pub zoom_level_policy: ZoomLevelPolicy,
    /// 実行するクエリ式
    pub query: QueryNode,
}
