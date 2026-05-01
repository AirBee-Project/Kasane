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
