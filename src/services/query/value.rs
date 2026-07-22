//! クエリで扱う値型の抽象。
//!
//! Kasane-Logic の作業木は単一の値型で組まれるため、テーブルの `data_type` ごとに
//! 具体型へ単型化する。演算子・`MergePolicy` の適用可否は型ごとに異なり、
//!
//! - 全型: `Overwrite` / `KeepExisting` / `Max` / `Min`（`Ord` があればよい）
//! - 数値型: 加えて `Sum` / `Difference` と `falloffLinear*`
//! - `From<u16>` を持つ数値型(i32/i64/浮動小数): 加えて `Average`
//!
//! という3階層になっている。使えない組合せは 400 を返す。

use core::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use kasane_logic::{
    CellValue, FlexTreeCore, Query,
    merge_policy::{Average, Difference, KeepExisting, Max, Min, Overwrite, Sum},
};

use crate::{
    error::AppError,
    models::{
        database::table::{JsonValueType, Table, TableConstraints, TableDataType},
        query::MergePolicyKind,
    },
};

/// 値型 `V` に対するクエリ AST。
pub type ValueQuery<V> = Query<FlexTreeCore<V>>;

/// 格納バイト列を値へ復元するデコーダ。`None` を返したセルは結果から除外される。
pub type Decoder<V> = Arc<dyn Fn(&[u8]) -> Option<V> + Send + Sync>;

// ---------------------------------------------------------------------------
// 浮動小数の Ord ラッパー
// ---------------------------------------------------------------------------

/// 浮動小数を `Ord` にするラッパーを定義する。
///
/// [`CellValue`] は `Ord` を要求するが浮動小数は `PartialOrd` しか持たないため、
/// `total_cmp` による全順序を与える（NaN でも panic しない）。
macro_rules! define_ordered_float {
    ($name:ident, $inner:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq, Default)]
        pub struct $name(pub $inner);

        impl Eq for $name {}
        impl Ord for $name {
            fn cmp(&self, other: &Self) -> Ordering {
                self.0.total_cmp(&other.0)
            }
        }
        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl core::ops::Mul for $name {
            type Output = Self;
            fn mul(self, rhs: Self) -> Self {
                $name(self.0 * rhs.0)
            }
        }
        impl core::ops::Div for $name {
            type Output = Self;
            fn div(self, rhs: Self) -> Self {
                $name(self.0 / rhs.0)
            }
        }
        impl core::ops::Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                $name(self.0 - rhs.0)
            }
        }
        impl kasane_logic::merge_policy::saturating_add::Add for $name {
            fn saturating_add(self, rhs: Self) -> Self {
                $name(self.0 + rhs.0)
            }
        }
        impl From<u16> for $name {
            fn from(v: u16) -> Self {
                $name(v as $inner)
            }
        }
        // `falloffLinear*` は `TryFrom<u32>` を要求するが、浮動小数への変換は必ず成功する。
        // `From` を実装すれば標準のブランケット実装経由で `TryFrom` も満たされる。
        impl From<u32> for $name {
            fn from(v: u32) -> Self {
                $name(v as $inner)
            }
        }
    };
}

define_ordered_float!(OrderedF32, f32);
define_ordered_float!(OrderedF64, f64);

// ---------------------------------------------------------------------------
// エラー
// ---------------------------------------------------------------------------

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

fn incompatible_source(table: &Table, value_type: &str) -> AppError {
    AppError::ConstraintViolation {
        reason: format!(
            "table '{}' is {:?} but the query value type is {}; add a `convert` table to this source",
            table.name, table.data_type, value_type
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

// ---------------------------------------------------------------------------
// QueryValue
// ---------------------------------------------------------------------------

/// クエリで扱える値型。
pub trait QueryValue: CellValue + Ord + 'static {
    /// エラーメッセージ用の型名。
    fn type_name() -> &'static str;

    /// このテーブルの格納値を `Self` へ復元するデコーダを返す。
    ///
    /// 型が合わなければ `Err`。`Enum` のように復元に制約情報が要る型もあるため、
    /// バイト列だけでなくテーブル定義を受け取る。
    fn decoder(table: &Table) -> Result<Decoder<Self>, AppError>;

    /// レスポンス用の JSON へ。
    fn to_json(&self) -> serde_json::Value;

    /// リクエスト中のリテラル（フィルタ境界・merge の既定値・変換表の値）から作る。
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

    /// 線形減衰。値の乗除算を要するため、既定では非対応。
    fn falloff_x(
        _q: ValueQuery<Self>,
        _z: u8,
        _r: u32,
        _p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError> {
        Err(unsupported_op("falloffLinearX", Self::type_name()))
    }
    fn falloff_y(
        _q: ValueQuery<Self>,
        _z: u8,
        _r: u32,
        _p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError> {
        Err(unsupported_op("falloffLinearY", Self::type_name()))
    }
    fn falloff_f(
        _q: ValueQuery<Self>,
        _z: u8,
        _r: u32,
        _p: MergePolicyKind,
    ) -> Result<ValueQuery<Self>, AppError> {
        Err(unsupported_op("falloffLinearF", Self::type_name()))
    }
}

// ---------------------------------------------------------------------------
// ポリシーのディスパッチ（値 -> 型）
// ---------------------------------------------------------------------------

/// 全型で使えるポリシーのみを受け付ける。
macro_rules! dispatch_ord {
    ($ty:ty, $q:expr, $method:ident ( $($args:expr),* ), $policy:expr) => {
        match $policy {
            MergePolicyKind::Overwrite => Ok($q.$method($($args,)* Overwrite)),
            MergePolicyKind::KeepExisting => Ok($q.$method($($args,)* KeepExisting)),
            MergePolicyKind::Max => Ok($q.$method($($args,)* Max)),
            MergePolicyKind::Min => Ok($q.$method($($args,)* Min)),
            other => Err(unsupported_policy(other, <$ty as QueryValue>::type_name())),
        }
    };
}

/// 数値型（`Average` は `From<u16>` が要るため除く）。
macro_rules! dispatch_numeric {
    ($ty:ty, $q:expr, $method:ident ( $($args:expr),* ), $policy:expr) => {
        match $policy {
            MergePolicyKind::Overwrite => Ok($q.$method($($args,)* Overwrite)),
            MergePolicyKind::KeepExisting => Ok($q.$method($($args,)* KeepExisting)),
            MergePolicyKind::Max => Ok($q.$method($($args,)* Max)),
            MergePolicyKind::Min => Ok($q.$method($($args,)* Min)),
            MergePolicyKind::Sum => Ok($q.$method($($args,)* Sum)),
            MergePolicyKind::Difference => Ok($q.$method($($args,)* Difference)),
            other => Err(unsupported_policy(other, <$ty as QueryValue>::type_name())),
        }
    };
}

/// `Average` まで含めた全ポリシー。
macro_rules! dispatch_full {
    ($q:expr, $method:ident ( $($args:expr),* ), $policy:expr) => {
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
// impl 生成マクロ
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

/// 算術が使える型に `falloffLinear*` を生やす。
macro_rules! impl_falloff {
    ($ty:ty, $dispatch:ident) => {
        fn falloff_x(
            q: ValueQuery<Self>,
            z: u8,
            r: u32,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, falloff_linear_x(z, r), p)
        }
        fn falloff_y(
            q: ValueQuery<Self>,
            z: u8,
            r: u32,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, falloff_linear_y(z, r), p)
        }
        fn falloff_f(
            q: ValueQuery<Self>,
            z: u8,
            r: u32,
            p: MergePolicyKind,
        ) -> Result<ValueQuery<Self>, AppError> {
            $dispatch!($ty, q, falloff_linear_f(z, r), p)
        }
    };
}

/// `Average` を含む全ポリシー用に、`$ty` を無視するラッパー。
macro_rules! dispatch_full_ty {
    ($ty:ty, $q:expr, $method:ident ( $($args:expr),* ), $policy:expr) => {
        dispatch_full!($q, $method($($args),*), $policy)
    };
}

/// 固定長ビッグエンディアン整数のデコーダ。
macro_rules! int_decoder {
    ($ty:ty, $n:expr) => {
        |bytes: &[u8]| <[u8; $n]>::try_from(bytes).ok().map(<$ty>::from_be_bytes)
    };
}

/// 整数型の `QueryValue` 実装を生成する。
macro_rules! impl_int_query_value {
    ($ty:ty, $name:expr, $data_type:expr, $n:expr, $dispatch:ident) => {
        impl QueryValue for $ty {
            fn type_name() -> &'static str {
                $name
            }

            fn decoder(table: &Table) -> Result<Decoder<Self>, AppError> {
                if table.data_type != $data_type {
                    return Err(incompatible_source(table, $name));
                }
                Ok(Arc::new(int_decoder!($ty, $n)))
            }

            fn to_json(&self) -> serde_json::Value {
                serde_json::Value::from(*self)
            }

            fn from_json(value: &serde_json::Value) -> Result<Self, AppError> {
                value
                    .as_i64()
                    .and_then(|v| <$ty>::try_from(v).ok())
                    .ok_or_else(|| AppError::NumericValueOutOfRange {
                        actual: value.to_string(),
                        expected: $name.to_string(),
                    })
            }

            impl_ops!($ty, $dispatch);
            impl_falloff!($ty, $dispatch);
        }
    };
}

/// 浮動小数型の `QueryValue` 実装を生成する。
macro_rules! impl_float_query_value {
    ($ty:ty, $inner:ty, $name:expr, $data_type:expr, $n:expr) => {
        impl QueryValue for $ty {
            fn type_name() -> &'static str {
                $name
            }

            fn decoder(table: &Table) -> Result<Decoder<Self>, AppError> {
                if table.data_type != $data_type {
                    return Err(incompatible_source(table, $name));
                }
                Ok(Arc::new(|bytes: &[u8]| {
                    <[u8; $n]>::try_from(bytes)
                        .ok()
                        .map(|b| Self(<$inner>::from_be_bytes(b)))
                }))
            }

            fn to_json(&self) -> serde_json::Value {
                serde_json::Number::from_f64(self.0 as f64)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            }

            fn from_json(value: &serde_json::Value) -> Result<Self, AppError> {
                let v = value
                    .as_f64()
                    .ok_or_else(|| AppError::NumericValueOutOfRange {
                        actual: value.to_string(),
                        expected: $name.to_string(),
                    })? as $inner;
                if v.is_finite() {
                    Ok(Self(v))
                } else {
                    Err(AppError::NumericValueOutOfRange {
                        actual: value.to_string(),
                        expected: concat!("finite ", $name).to_string(),
                    })
                }
            }

            impl_ops!($ty, dispatch_full_ty);
            impl_falloff!($ty, dispatch_full_ty);
        }
    };
}

// 整数: TinyInt / SmallInt は `From<u16>` を持たないため Average を使えない。
impl_int_query_value!(i8, "TinyInt", TableDataType::TinyInt, 1, dispatch_numeric);
impl_int_query_value!(
    i16,
    "SmallInt",
    TableDataType::SmallInt,
    2,
    dispatch_numeric
);
impl_int_query_value!(i32, "Int", TableDataType::Int, 4, dispatch_full_ty);
impl_int_query_value!(i64, "BigInt", TableDataType::BigInt, 8, dispatch_full_ty);

// 浮動小数
impl_float_query_value!(OrderedF32, f32, "Float", TableDataType::Float, 4);
impl_float_query_value!(OrderedF64, f64, "Double", TableDataType::Double, 8);

// ---------------------------------------------------------------------------
// 非数値型
// ---------------------------------------------------------------------------

impl QueryValue for String {
    fn type_name() -> &'static str {
        "Text"
    }

    /// `Text` はそのまま UTF-8 として、`Enum` は制約の対応表から文字列へ復元する。
    fn decoder(table: &Table) -> Result<Decoder<Self>, AppError> {
        match table.data_type {
            TableDataType::Text => Ok(Arc::new(|bytes: &[u8]| {
                core::str::from_utf8(bytes).ok().map(|s| s.to_string())
            })),
            TableDataType::Enum => {
                let Some(TableConstraints::Enum { mapping, .. }) = table.constraints.as_ref()
                else {
                    return Err(AppError::InvalidStoredValue {
                        reason: format!("table '{}' is Enum but has no choice mapping", table.name),
                    });
                };
                // 格納は u16 ID なので、ID -> 文字列の逆引きを作っておく。
                let reverse: HashMap<u16, String> =
                    mapping.iter().map(|(k, &v)| (v, k.clone())).collect();
                Ok(Arc::new(move |bytes: &[u8]| {
                    let id = u16::from_be_bytes(<[u8; 2]>::try_from(bytes).ok()?);
                    reverse.get(&id).cloned()
                }))
            }
            _ => Err(incompatible_source(table, "Text")),
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

impl QueryValue for bool {
    fn type_name() -> &'static str {
        "Boolean"
    }

    fn decoder(table: &Table) -> Result<Decoder<Self>, AppError> {
        if table.data_type != TableDataType::Boolean {
            return Err(incompatible_source(table, "Boolean"));
        }
        Ok(Arc::new(|bytes: &[u8]| match bytes {
            [0] => Some(false),
            [1] => Some(true),
            _ => None,
        }))
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
impl QueryValue for () {
    fn type_name() -> &'static str {
        "Presence"
    }

    fn decoder(table: &Table) -> Result<Decoder<Self>, AppError> {
        if table.data_type != TableDataType::Presence {
            return Err(incompatible_source(table, "Presence"));
        }
        // 格納は 0 バイト。存在すれば値は常に「無」。
        Ok(Arc::new(
            |bytes: &[u8]| if bytes.is_empty() { Some(()) } else { None },
        ))
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
