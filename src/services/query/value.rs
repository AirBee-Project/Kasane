use std::collections::HashMap;
use std::sync::Arc;

use kasane_logic::{
    Query, SafeValue,
    merge_policy::{Average, Difference, KeepExisting, Max, Min, Overwrite, Sum},
};

use crate::repositories::traits::DecodeFn;
use crate::{
    error::AppError,
    models::{
        ValueLiteral, ValueType,
        database::table::{TableConstraints, TableDataType},
        query::MergePolicyKind,
    },
};

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

fn type_mismatch(value: &ValueLiteral, expected: ValueType) -> AppError {
    AppError::ValueTypeMismatch {
        actual: value.value_type(),
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

// --- Value ---

/// アプリで扱える値型。格納・復元・リテラル変換・クエリ演算を型ごとに引き受ける。
pub trait Value: SafeValue + Ord + 'static {
    /// エラーメッセージ用の型名。
    fn type_name() -> &'static str;

    /// `for_value_type!` の分岐と一致させること（`Text` と `Enum` はともに `String`）。
    fn accepts(data_type: TableDataType) -> bool;

    /// 制約を受け取るのは `Enum` の復元に ID → 文字列の対応が要るため。逆引き表の前計算を
    /// 1 度だけ行ったクロージャを返す。`None` を返した FlexId は結果から除外される。
    fn decoder(constraints: Option<&TableConstraints>) -> Result<DecodeFn<Self>, AppError>;

    /// `Self` を格納バイト列へ符号化する（`constraints` の範囲・選択肢検証込み）。
    fn encode(&self, constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError>;

    /// レスポンス用の [`ValueLiteral`] へ。
    fn to_value(&self) -> ValueLiteral;

    /// リクエスト中のリテラル（挿入値・フィルタ境界・merge の既定値）から作る。
    fn from_value(value: &ValueLiteral) -> Result<Self, AppError>;

    fn zoom_out(q: Query<Self>, z: u8, p: MergePolicyKind) -> Result<Query<Self>, AppError>;

    fn extrude_x(
        q: Query<Self>,
        z: u8,
        start: u32,
        end: u32,
        p: MergePolicyKind,
    ) -> Result<Query<Self>, AppError>;

    fn extrude_y(
        q: Query<Self>,
        z: u8,
        start: u32,
        end: u32,
        p: MergePolicyKind,
    ) -> Result<Query<Self>, AppError>;

    fn extrude_f(
        q: Query<Self>,
        z: u8,
        start: i32,
        end: i32,
        p: MergePolicyKind,
    ) -> Result<Query<Self>, AppError>;

    fn merge(
        lhs: Query<Self>,
        rhs: Query<Self>,
        default: Self,
        p: MergePolicyKind,
    ) -> Result<Query<Self>, AppError>;

    /// 値の減衰。値の乗除算を要するため、既定では非対応。
    fn falloff_x(
        _q: Query<Self>,
        _z: u8,
        _r: u32,
        _direction: Option<kasane_logic::spatial_id::helpers::Side>,
        _pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
        _p: MergePolicyKind,
    ) -> Result<Query<Self>, AppError> {
        Err(unsupported_op("falloffX", Self::type_name()))
    }
    fn falloff_y(
        _q: Query<Self>,
        _z: u8,
        _r: u32,
        _direction: Option<kasane_logic::spatial_id::helpers::Side>,
        _pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
        _p: MergePolicyKind,
    ) -> Result<Query<Self>, AppError> {
        Err(unsupported_op("falloffY", Self::type_name()))
    }
    fn falloff_f(
        _q: Query<Self>,
        _z: u8,
        _r: u32,
        _direction: Option<kasane_logic::spatial_id::helpers::Side>,
        _pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
        _p: MergePolicyKind,
    ) -> Result<Query<Self>, AppError> {
        Err(unsupported_op("falloffF", Self::type_name()))
    }

    /// 四則演算。既定では非対応。
    fn apply_math(
        _q: Query<Self>,
        _op: crate::models::query::MathOperator,
        _operand: crate::models::query::MathOperand,
    ) -> Result<Query<Self>, AppError> {
        Err(unsupported_op("math operation", Self::type_name()))
    }
}

// --- ポリシーのディスパッチ（値 -> 型） ---

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

// --- op 生成マクロ ---

/// 演算子メソッド群を、指定のポリシーディスパッチで生成する。
macro_rules! impl_ops {
    ($ty:ty, $dispatch:ident) => {
        fn zoom_out(q: Query<Self>, z: u8, p: MergePolicyKind) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, q, zoom_out(z), p)
        }
        fn extrude_x(
            q: Query<Self>,
            z: u8,
            start: u32,
            end: u32,
            p: MergePolicyKind,
        ) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, q, extrude_x(z, start, end), p)
        }
        fn extrude_y(
            q: Query<Self>,
            z: u8,
            start: u32,
            end: u32,
            p: MergePolicyKind,
        ) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, q, extrude_y(z, start, end), p)
        }
        fn extrude_f(
            q: Query<Self>,
            z: u8,
            start: i32,
            end: i32,
            p: MergePolicyKind,
        ) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, q, extrude_f(z, start, end), p)
        }
        fn merge(
            lhs: Query<Self>,
            rhs: Query<Self>,
            default: Self,
            p: MergePolicyKind,
        ) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, lhs, merge(rhs, default), p)
        }
    };
}

/// 算術が使える型に `falloff*` を生やす。
macro_rules! impl_falloff {
    ($ty:ty, $dispatch:ident) => {
        fn falloff_x(
            q: Query<Self>,
            z: u8,
            r: u32,
            direction: Option<kasane_logic::spatial_id::helpers::Side>,
            pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
            p: MergePolicyKind,
        ) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, q, falloff_x(z, r, direction, pattern), p)
        }
        fn falloff_y(
            q: Query<Self>,
            z: u8,
            r: u32,
            direction: Option<kasane_logic::spatial_id::helpers::Side>,
            pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
            p: MergePolicyKind,
        ) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, q, falloff_y(z, r, direction, pattern), p)
        }
        fn falloff_f(
            q: Query<Self>,
            z: u8,
            r: u32,
            direction: Option<kasane_logic::spatial_id::helpers::Side>,
            pattern: kasane_logic::spatial_id::collection::query::ops::unary::falloff::FalloffPattern,
            p: MergePolicyKind,
        ) -> Result<Query<Self>, AppError> {
            $dispatch!($ty, q, falloff_f(z, r, direction, pattern), p)
        }
    };
}

// --- 数値型 ---

impl Value for i64 {
    fn type_name() -> &'static str {
        "Int"
    }

    fn accepts(data_type: TableDataType) -> bool {
        data_type == TableDataType::Int
    }

    fn decoder(_constraints: Option<&TableConstraints>) -> Result<DecodeFn<Self>, AppError> {
        Ok(Arc::new(|bytes: &[u8]| {
            <[u8; 8]>::try_from(bytes).ok().map(Self::from_be_bytes)
        }))
    }

    fn encode(&self, constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        if let Some(TableConstraints::Int { min, max }) = constraints {
            check_range(self, *min, *max)?;
        }
        Ok(self.to_be_bytes().to_vec())
    }

    fn to_value(&self) -> ValueLiteral {
        ValueLiteral::Int(*self)
    }

    fn from_value(value: &ValueLiteral) -> Result<Self, AppError> {
        value
            .as_i64()
            .ok_or_else(|| type_mismatch(value, ValueType::Int))
    }

    impl_ops!(i64, dispatch_full);
    impl_falloff!(i64, dispatch_full);

    fn apply_math(
        q: Query<Self>,
        op: crate::models::query::MathOperator,
        operand: crate::models::query::MathOperand,
    ) -> Result<Query<Self>, AppError> {
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
                    Ok(q.map_values(move |v| v.checked_div(i_op).unwrap_or(Self::MAX)))
                }
            },
            MathOperand::Float(f_op) => {
                if f_op.fract() == 0.0 && f_op >= (Self::MIN as f64) && f_op <= (Self::MAX as f64) {
                    let i_op = f_op as Self;
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
                        if res.is_nan() { 0 } else { res.round() as Self }
                    })),
                    MathOperator::Subtract => Ok(q.map_values(move |v| {
                        let res = (v as f64) - f_op;
                        if res.is_nan() { 0 } else { res.round() as Self }
                    })),
                    MathOperator::Multiply => Ok(q.map_values(move |v| {
                        let res = (v as f64) * f_op;
                        if res.is_nan() { 0 } else { res.round() as Self }
                    })),
                    MathOperator::Divide => Ok(q.map_values(move |v| {
                        let res = (v as f64) / f_op;
                        if res.is_nan() { 0 } else { res.round() as Self }
                    })),
                }
            }
        }
    }
}

// --- 非数値型 ---

impl Value for String {
    fn type_name() -> &'static str {
        "Text"
    }

    fn accepts(data_type: TableDataType) -> bool {
        matches!(data_type, TableDataType::Text | TableDataType::Enum)
    }

    /// `Enum`（制約あり）は ID→文字列の対応表から、それ以外は UTF-8 として復元する。
    fn decoder(constraints: Option<&TableConstraints>) -> Result<DecodeFn<Self>, AppError> {
        if let Some(TableConstraints::Enum { mapping, .. }) = constraints {
            // 格納は u16 ID なので、ID -> 文字列の逆引きを 1 度だけ作る。
            let reverse: HashMap<u16, Self> =
                mapping.iter().map(|(k, &v)| (v, k.clone())).collect();
            Ok(Arc::new(move |bytes: &[u8]| {
                let id = u16::from_be_bytes(<[u8; 2]>::try_from(bytes).ok()?);
                reverse.get(&id).cloned()
            }))
        } else {
            Ok(Arc::new(|bytes: &[u8]| {
                core::str::from_utf8(bytes)
                    .ok()
                    .map(std::string::ToString::to_string)
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
                    .ok_or_else(|| {
                        AppError::InternalError(format!(
                            "enum value '{self}' has no id in the constraint mapping"
                        ))
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

    fn to_value(&self) -> ValueLiteral {
        ValueLiteral::String(self.clone())
    }

    fn from_value(value: &ValueLiteral) -> Result<Self, AppError> {
        value
            .as_str()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| type_mismatch(value, ValueType::String))
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

    fn decoder(_constraints: Option<&TableConstraints>) -> Result<DecodeFn<Self>, AppError> {
        Ok(Arc::new(|bytes: &[u8]| match bytes {
            [0] => Some(false),
            [1] => Some(true),
            _ => None,
        }))
    }

    fn encode(&self, _constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        Ok(vec![*self as u8])
    }

    fn to_value(&self) -> ValueLiteral {
        ValueLiteral::Bool(*self)
    }

    fn from_value(value: &ValueLiteral) -> Result<Self, AppError> {
        value
            .as_bool()
            .ok_or_else(|| type_mismatch(value, ValueType::Bool))
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

    fn decoder(_constraints: Option<&TableConstraints>) -> Result<DecodeFn<Self>, AppError> {
        // 格納は 0 バイト。存在すれば値は常に「無」。
        Ok(Arc::new(
            |bytes: &[u8]| if bytes.is_empty() { Some(()) } else { None },
        ))
    }

    fn encode(&self, _constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        Ok(Vec::new())
    }

    fn to_value(&self) -> ValueLiteral {
        ValueLiteral::Null
    }

    fn from_value(value: &ValueLiteral) -> Result<Self, AppError> {
        if value.is_null() {
            Ok(())
        } else {
            Err(type_mismatch(value, ValueType::Null))
        }
    }

    impl_ops!((), dispatch_ord);
}
