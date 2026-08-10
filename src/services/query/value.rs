use std::collections::HashMap;
use std::sync::Arc;

use kasane_logic::{
    Query, SafeValue,
    merge_policy::{Average, Difference, KeepExisting, Max, Min, Overwrite, Sum},
};

use crate::{
    error::AppError,
    models::{
        database::table::{JsonValueType, TableConstraints, TableDataType},
        query::MergePolicyKind,
    },
};

/// 値型 `V` に対するクエリ AST。
pub type ValueQuery<V> = Query<V>;

/// 格納バイト列を値へ復元するデコーダ。`None` を返したセルは結果から除外される。
pub type Decoder<V> = Arc<dyn Fn(&[u8]) -> Option<V> + Send + Sync>;

#[macro_export]
macro_rules! for_value_type {
    ($dt:expr, $func:ident $([$($rest:ty),* $(,)?])? $(, $arg:expr)* $(,)?) => {{
        use $crate::models::database::table::TableDataType;
        match $dt {
            TableDataType::Int => $func::<i64 $($(, $rest)*)?>($($arg),*),
            TableDataType::Text | TableDataType::Enum => $func::<String $($(, $rest)*)?>($($arg),*),
            TableDataType::Boolean => $func::<bool $($(, $rest)*)?>($($arg),*),
            TableDataType::Presence => $func::<() $($(, $rest)*)?>($($arg),*),
        }
    }};
}

fn unsupported_op(op: &str, value_type: &str) -> AppError {
    AppError::ConstraintViolation {
        reason: format!("operator '{op}' is not applicable to {value_type} values"),
    }
}

fn unsupported_policy(policy: MergePolicyKind, value_type: &str) -> AppError {
    AppError::ConstraintViolation {
        reason: format!("merge policy '{policy:?}' is not applicable to {value_type} values"),
    }
}

/// ソースの `data_type` がクエリ値型として読めないときのエラー。
#[tracing::instrument(skip_all)]
pub fn incompatible_source(
    table: &crate::models::database::table::Table,
    value_type: &str,
) -> AppError {
    AppError::ConstraintViolation {
        reason: format!(
            "table '{}' is {:?}, which cannot be read as the query value type {value_type}",
            table.name, table.data_type
        ),
    }
}

/// リクエスト中の JSON リテラルの種別（エラー報告用）。
fn json_value_type(value: &serde_json::Value) -> JsonValueType {
    match value {
        serde_json::Value::String(_) => JsonValueType::String,
        serde_json::Value::Number(_) => JsonValueType::Number,
        serde_json::Value::Bool(_) => JsonValueType::Bool,
        serde_json::Value::Array(_) => JsonValueType::Array,
        serde_json::Value::Object(_) => JsonValueType::Object,
        serde_json::Value::Null => JsonValueType::Null,
    }
}

fn type_mismatch(value: &serde_json::Value, expected: JsonValueType) -> AppError {
    AppError::ValueTypeMismatch {
        actual: json_value_type(value),
        expected,
    }
}

/// `min`/`max` 範囲の検証（境界を含む）。
fn check_range<T: PartialOrd + std::fmt::Display>(
    value: &T,
    min: Option<T>,
    max: Option<T>,
) -> Result<(), AppError> {
    if let Some(min) = min
        && *value < min
    {
        return Err(AppError::ConstraintViolation {
            reason: format!("Value {value} is less than minimum {min}"),
        });
    }
    if let Some(max) = max
        && *value > max
    {
        return Err(AppError::ConstraintViolation {
            reason: format!("Value {value} is greater than maximum {max}"),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Value
// ---------------------------------------------------------------------------

/// アプリで扱える値型。格納・復元・JSON 変換・クエリ演算を型ごとに引き受ける。
pub trait Value: SafeValue + Ord + 'static {
    /// エラーメッセージ用の型名。
    fn type_name() -> &'static str;

    /// この `data_type` の格納値を `Self` として解釈できるか。
    ///
    /// `for_value_type!` の分岐と一致させる（`Text` と `Enum` はともに `String`）。
    fn accepts(data_type: TableDataType) -> bool;

    /// このテーブルの格納バイト列を `Self` へ復元するデコーダを返す。
    ///
    /// `Enum` のように復元に制約情報（ID→文字列の対応）が要る型もあるため制約を受け取り、
    /// 逆引き表などの前計算を 1 度だけ行ったクロージャを返す。`None` を返したセルは除外。
    fn decoder(constraints: Option<&TableConstraints>) -> Result<Decoder<Self>, AppError>;

    /// `Self` を格納バイト列へ符号化する（`constraints` の範囲・選択肢検証込み）。
    fn encode(&self, constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError>;

    /// レスポンス用の JSON へ。
    fn to_json(&self) -> serde_json::Value;

    /// リクエスト中のリテラル（挿入値・フィルタ境界・merge の既定値）から作る。
    fn from_json(value: &serde_json::Value) -> Result<Self, AppError>;

    fn zoom_out(
        q: ValueQuery<Self>,
        z: u8,
        p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError>;

    fn extrude_x(
        q: ValueQuery<Self>,
        z: u8,
        start: u32,
        end: u32,
        p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError>;

    fn extrude_y(
        q: ValueQuery<Self>,
        z: u8,
        start: u32,
        end: u32,
        p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError>;

    fn extrude_f(
        q: ValueQuery<Self>,
        z: u8,
        start: i32,
        end: i32,
        p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError>;

    fn merge(
        lhs: ValueQuery<Self>,
        rhs: ValueQuery<Self>,
        default: Self,
        p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError>;

    /// 値の減衰。値の乗除算を要するため、既定では非対応。
    fn falloff_x(
        _q: ValueQuery<Self>,
        _z: u8,
        _r: u32,
        _direction: Option<kasane_logic::spatial_id::helpers::Side>,
        _pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
        _p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError> {
        Err(unsupported_op("falloffX", Self::type_name()))
    }
    fn falloff_y(
        _q: ValueQuery<Self>,
        _z: u8,
        _r: u32,
        _direction: Option<kasane_logic::spatial_id::helpers::Side>,
        _pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
        _p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError> {
        Err(unsupported_op("falloffY", Self::type_name()))
    }
    fn falloff_f(
        _q: ValueQuery<Self>,
        _z: u8,
        _r: u32,
        _direction: Option<kasane_logic::spatial_id::helpers::Side>,
        _pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
        _p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError> {
        Err(unsupported_op("falloffF", Self::type_name()))
    }

    /// 四則演算。既定では非対応。
    fn apply_math(
        _q: ValueQuery<Self>,
        _op: crate::models::query::MathOperator,
        _operand: crate::models::query::MathOperand,
    ) -> Result<ValueQuery<Self>, AppError> {
        Err(unsupported_op("math operation", Self::type_name()))
    }
}

// ---------------------------------------------------------------------------
// ポリシーのディスパッチ（値 -> 型）
// ---------------------------------------------------------------------------

/// 全型で使えるポリシー（`Ord` があればよい）。
macro_rules! dispatch_ord {
    ($ty:ty, $q:expr, $method:ident ( $($args:expr),* ), $policy:expr) => {
        match $policy {
            MergePolicyKind::Overwrite => Ok($q.$method($($args,)* Overwrite)),
            MergePolicyKind::KeepExisting => Ok($q.$method($($args,)* KeepExisting)),
            MergePolicyKind::Max => Ok($q.$method($($args,)* Max)),
            MergePolicyKind::Min => Ok($q.$method($($args,)* Min)),
            other => Err(unsupported_policy(other, <$ty as Value>::type_name())),
        }
    };
}

/// 数値型で使える全ポリシー。
macro_rules! dispatch_full {
    ($ty:ty, $q:expr, $method:ident ( $($args:expr),* ), $policy:expr) => {
        match $policy {
            MergePolicyKind::Overwrite => Ok($q.$method($($args,)* Overwrite)),
            MergePolicyKind::KeepExisting => Ok($q.$method($($args,)* KeepExisting)),
            MergePolicyKind::Max => Ok($q.$method($($args,)* Max)),
            MergePolicyKind::Min => Ok($q.$method($($args,)* Min)),
            MergePolicyKind::Sum => Ok($q.$method($($args,)* Sum)),
            MergePolicyKind::Difference => Ok($q.$method($($args,)* Difference)),
            MergePolicyKind::Average => Ok($q.$method($($args,)* Average)),
        }
    };
}

// ---------------------------------------------------------------------------
// op 生成マクロ
// ---------------------------------------------------------------------------

/// 演算子メソッド群を、指定のポリシーディスパッチで生成する。
macro_rules! impl_ops {
    ($ty:ty, $dispatch:ident) => {
        fn zoom_out(
            q: ValueQuery<Self>,
            z: u8,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, zoom_out(z), p)
        }
        fn extrude_x(
            q: ValueQuery<Self>,
            z: u8,
            start: u32,
            end: u32,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, extrude_x(z, start, end), p)
        }
        fn extrude_y(
            q: ValueQuery<Self>,
            z: u8,
            start: u32,
            end: u32,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, extrude_y(z, start, end), p)
        }
        fn extrude_f(
            q: ValueQuery<Self>,
            z: u8,
            start: i32,
            end: i32,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, extrude_f(z, start, end), p)
        }
        fn merge(
            lhs: ValueQuery<Self>,
            rhs: ValueQuery<Self>,
            default: Self,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, lhs, merge(rhs, default), p)
        }
    };
}

/// 算術が使える型に `falloff*` を生やす。
macro_rules! impl_falloff {
    ($ty:ty, $dispatch:ident) => {
        fn falloff_x(
            q: ValueQuery<Self>,
            z: u8,
            r: u32,
            direction: Option<kasane_logic::spatial_id::helpers::Side>,
            pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, falloff_x(z, r, direction, pattern), p)
        }
        fn falloff_y(
            q: ValueQuery<Self>,
            z: u8,
            r: u32,
            direction: Option<kasane_logic::spatial_id::helpers::Side>,
            pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, falloff_y(z, r, direction, pattern), p)
        }
        fn falloff_f(
            q: ValueQuery<Self>,
            z: u8,
            r: u32,
            direction: Option<kasane_logic::spatial_id::helpers::Side>,
            pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, falloff_f(z, r, direction, pattern), p)
        }
    };
}

// ---------------------------------------------------------------------------
// 数値型
// ---------------------------------------------------------------------------

impl Value for i64 {
    fn type_name() -> &'static str {
        "Int"
    }

    fn accepts(data_type: TableDataType) -> bool {
        data_type == TableDataType::Int
    }

    fn decoder(_constraints: Option<&TableConstraints>) -> Result<Decoder<Self>, AppError> {
        Ok(Arc::new(|bytes: &[u8]| {
            <[u8; 8]>::try_from(bytes).ok().map(i64::from_be_bytes)
        }))
    }

    fn encode(&self, constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        if let Some(TableConstraints::Int { min, max }) = constraints {
            check_range(self, *min, *max)?;
        }
        Ok(self.to_be_bytes().to_vec())
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::Value::from(*self)
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, AppError> {
        value
            .as_i64()
            .ok_or_else(|| AppError::NumericValueOutOfRange {
                actual: value.to_string(),
                expected: "Int".to_string(),
            })
    }

    impl_ops!(i64, dispatch_full);
    impl_falloff!(i64, dispatch_full);

    fn apply_math(
        q: ValueQuery<Self>,
        op: crate::models::query::MathOperator,
        operand: crate::models::query::MathOperand,
    ) -> Result<ValueQuery<Self>, AppError> {
        use crate::models::query::{MathOperand, MathOperator};
        match operand {
            MathOperand::Int(i_op) => match op {
                MathOperator::Add => Ok(q.map_values(move |v| v.saturating_add(i_op))),
                MathOperator::Subtract => Ok(q.map_values(move |v| v.saturating_sub(i_op))),
                MathOperator::Multiply => Ok(q.map_values(move |v| v.saturating_mul(i_op))),
                MathOperator::Divide => {
                    if i_op == 0 {
                        return Err(AppError::ConstraintViolation {
                            reason: "Division by zero".to_string(),
                        });
                    }
                    Ok(q.map_values(move |v| v.checked_div(i_op).unwrap_or(i64::MAX)))
                }
            },
            MathOperand::Float(f_op) => {
                if f_op.fract() == 0.0 && f_op >= (i64::MIN as f64) && f_op <= (i64::MAX as f64) {
                    let i_op = f_op as i64;
                    return Self::apply_math(q, op, MathOperand::Int(i_op));
                }

                if op == MathOperator::Divide && f_op == 0.0 {
                    return Err(AppError::ConstraintViolation {
                        reason: "Division by zero".to_string(),
                    });
                }
                match op {
                    MathOperator::Add => Ok(q.map_values(move |v| {
                        let res = (v as f64) + f_op;
                        if res.is_nan() { 0 } else { res.round() as i64 }
                    })),
                    MathOperator::Subtract => Ok(q.map_values(move |v| {
                        let res = (v as f64) - f_op;
                        if res.is_nan() { 0 } else { res.round() as i64 }
                    })),
                    MathOperator::Multiply => Ok(q.map_values(move |v| {
                        let res = (v as f64) * f_op;
                        if res.is_nan() { 0 } else { res.round() as i64 }
                    })),
                    MathOperator::Divide => Ok(q.map_values(move |v| {
                        let res = (v as f64) / f_op;
                        if res.is_nan() { 0 } else { res.round() as i64 }
                    })),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 非数値型
// ---------------------------------------------------------------------------

impl Value for String {
    fn type_name() -> &'static str {
        "Text"
    }

    fn accepts(data_type: TableDataType) -> bool {
        matches!(data_type, TableDataType::Text | TableDataType::Enum)
    }

    /// `Enum`（制約あり）は ID→文字列の対応表から、それ以外は UTF-8 として復元する。
    fn decoder(constraints: Option<&TableConstraints>) -> Result<Decoder<Self>, AppError> {
        if let Some(TableConstraints::Enum { mapping, .. }) = constraints {
            // 格納は u16 ID なので、ID -> 文字列の逆引きを 1 度だけ作る。
            let reverse: HashMap<u16, String> =
                mapping.iter().map(|(k, &v)| (v, k.clone())).collect();
            Ok(Arc::new(move |bytes: &[u8]| {
                let id = u16::from_be_bytes(<[u8; 2]>::try_from(bytes).ok()?);
                reverse.get(&id).cloned()
            }))
        } else {
            Ok(Arc::new(|bytes: &[u8]| {
                core::str::from_utf8(bytes).ok().map(|s| s.to_string())
            }))
        }
    }

    /// `Enum`（制約あり）は選択肢を検証して u16 ID へ、それ以外は長さを検証して UTF-8 へ。
    fn encode(&self, constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        match constraints {
            Some(TableConstraints::Enum {
                choices, mapping, ..
            }) => {
                if !choices.contains(self) {
                    return Err(AppError::ConstraintViolation {
                        reason: format!("Value '{self}' is not among allowed choices: {choices:?}"),
                    });
                }
                mapping
                    .get(self)
                    .map(|id| id.to_be_bytes().to_vec())
                    .ok_or_else(|| AppError::InvalidStoredValue {
                        reason: format!("Internal mapping not found for enum value '{self}'"),
                    })
            }
            Some(TableConstraints::Text {
                min_length,
                max_length,
            }) => {
                let len = self.chars().count();
                if let Some(min) = min_length
                    && len < *min
                {
                    return Err(AppError::ConstraintViolation {
                        reason: format!("String length {len} is less than minimum length {min}"),
                    });
                }
                if let Some(max) = max_length
                    && len > *max
                {
                    return Err(AppError::ConstraintViolation {
                        reason: format!("String length {len} is greater than maximum length {max}"),
                    });
                }
                Ok(self.clone().into_bytes())
            }
            _ => Ok(self.clone().into_bytes()),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::Value::String(self.clone())
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, AppError> {
        value
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| type_mismatch(value, JsonValueType::String))
    }

    impl_ops!(String, dispatch_ord);
}

impl Value for bool {
    fn type_name() -> &'static str {
        "Boolean"
    }

    fn accepts(data_type: TableDataType) -> bool {
        data_type == TableDataType::Boolean
    }

    fn decoder(_constraints: Option<&TableConstraints>) -> Result<Decoder<Self>, AppError> {
        Ok(Arc::new(|bytes: &[u8]| match bytes {
            [0] => Some(false),
            [1] => Some(true),
            _ => None,
        }))
    }

    fn encode(&self, _constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        Ok(vec![*self as u8])
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Bool(*self)
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, AppError> {
        value
            .as_bool()
            .ok_or_else(|| type_mismatch(value, JsonValueType::Bool))
    }

    impl_ops!(bool, dispatch_ord);
}

/// `Presence`（値を持たず、空間IDの存在だけを表す型）。
impl Value for () {
    fn type_name() -> &'static str {
        "Presence"
    }

    fn accepts(data_type: TableDataType) -> bool {
        data_type == TableDataType::Presence
    }

    fn decoder(_constraints: Option<&TableConstraints>) -> Result<Decoder<Self>, AppError> {
        // 格納は 0 バイト。存在すれば値は常に「無」。
        Ok(Arc::new(
            |bytes: &[u8]| if bytes.is_empty() { Some(()) } else { None },
        ))
    }

    fn encode(&self, _constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        Ok(Vec::new())
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, AppError> {
        if value.is_null() {
            Ok(())
        } else {
            Err(type_mismatch(value, JsonValueType::Null))
        }
    }

    impl_ops!((), dispatch_ord);
}
