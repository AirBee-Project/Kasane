pub mod query;

use crate::models::table::query::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, ToSchema, Clone)]
///Table内の時空間IDに付与する値の型を指定する
/// 型の名前はMySQLと同じ命名規則を採用
pub enum TableDataType {
    ///Rustの[String]に対応
    Text,
    ///Rustの[i32]に対応
    Int,
    ///Rustの[f32]に対応
    Float,
    ///Rustの[bool]に対応
    Boolean,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDと値が対応するTableを作成する
pub struct CreateTableRequest {
    pub name: String,
    pub r#type: TableDataType,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///Tableを削除する
pub struct DropTableRequest {
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して取得する
/// 複数のテーブルを結合し、計算結果を取得するための高度なクエリリクエスト
pub struct SelectTableRequest {
    /// データソースの定義 (FROM / JOIN に相当)
    pub sources: Vec<TableSource>,
    /// どの空間範囲を対象にするか (WHERE に相当)
    pub spatial_query: Query,
    /// どのような値を計算して返すか (SELECT に相当)
    pub projection: Vec<ProjectionField>,
}

#[derive(Debug, Deserialize, ToSchema)]
/// クエリ内で使用するデータソース（テーブル）のエイリアス定義
pub struct TableSource {
    /// 実際のテーブル名
    pub name: String,
    /// クエリ内で参照するための別名 (例: "hazard_map", "population")
    pub alias: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum ProjectionField {
    /// テーブルの生の値をそのまま返す
    RawField {
        /// 参照するデータソースのエイリアス
        source_alias: String,
        /// 出力結果のJSONに付与するキー名 (省略時はエイリアス名と同等)
        as_name: Option<String>,
    },
    /// 計算式によって導出されたカスタム項目
    Calculated {
        /// 出力結果のJSONに付与するキー名 (例: "risk_score")
        name: String,
        /// 計算ロジック
        expression: Expression,
    },
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type")]
/// 計算式 (Expression DSL)
pub enum Expression {
    /// 条件分岐 (IF condition THEN then_value ELSE else_value)
    Case {
        /// 既存の `Query` を条件式として再利用 (特定の領域内か、特定の属性値を持つか)
        condition: Box<Query>,
        then_value: Box<Expression>,
        else_value: Box<Expression>,
    },
    /// 加算
    Add {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    /// 減算
    Subtract {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    /// 定数値 (整数)
    LiteralInt { value: i32 },
    /// 定数値 (浮動小数点)
    LiteralFloat { value: f32 },
    /// 定数値 (真偽値)
    LiteralBool { value: bool },
    /// 特定のデータソース（エイリアス）の値への参照
    SourceValue { alias: String },
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を挿入する
pub struct InsertTableRequest<V> {
    pub name: String,
    pub value: V,
    pub query: Query,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
///時空間IDの範囲を[Query]で指定して値を削除する
pub struct DeleteTableRequest {
    pub name: String,
    pub query: Query,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct InfoTableResponse {
    pub name: String,
    pub r#type: TableDataType,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
/// テーブルの現在の状態（メタデータ）を表すドメインモデル
pub struct TableInfo {
    pub name: String,
    pub r#type: TableDataType,
}
