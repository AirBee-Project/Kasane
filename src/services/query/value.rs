//! アプリ全体で使う値型の抽象（`Value` trait）。
//!
//! テーブルの `data_type` は実行時の値だが、Kasane-Logic の作業木や格納・復元は
//! 具体的な Rust 型で行うため、`data_type` → 具体型の単型化を [`for_value_type!`](crate::for_value_type) の
//! 1 箇所に集約する。各具体型は [`Value`] を実装し、
//!
//! - 格納バイト列 ⇄ 値（[`Value::decoder`] / [`Value::encode`]）
//! - JSON ⇄ 値（[`Value::from_json`] / [`Value::to_json`]）
//! - クエリ演算子（`zoom_out` などと `MergePolicy` の適用可否）
//!
//! を型ごとに一手に引き受ける。`search`（格納値の復元）と `query`（演算結果）は
//! いずれもこの trait を通すので、型ごとの処理を二重に書かずに済む。
//!
//! 演算子・`MergePolicy` の適用可否は 2 階層：
//! - 全型: `Overwrite` / `KeepExisting` / `Max` / `Min`（`Ord` があればよい）
//! - 数値型（`Int` = i64 / `Float` = f64）: 加えて `Sum` / `Difference` / `Average` と `falloffLinear*`
//!
//! 使えない組合せは 400 を返す。

use core::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use kasane_logic::{
    CellValue, Query,
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

/// `TableDataType` を対応する具体型へ単型化し、`$func::<V>(..)` を呼ぶ。
///
/// 「実行時の型 → コンパイル時の型」の分岐はここ 1 箇所だけ。型を増減するときも
/// この match だけを直せばよい。
///
/// 呼ぶ関数が型引数を 2 つ以上取る場合は、残りを `[..]` で後ろへ続ける
/// （例: `for_value_type!(dt, f[V], args..)` は `f::<単型化した型, V>(args..)`）。
#[macro_export]
macro_rules! for_value_type {
    ($dt:expr, $func:ident $([$($rest:ty),* $(,)?])? $(, $arg:expr)* $(,)?) => {{
        use $crate::models::database::table::TableDataType;
        match $dt {
            TableDataType::Int => $func::<i64 $($(, $rest)*)?>($($arg),*),
            TableDataType::Float => {
                $func::<$crate::services::query::value::OrderedFloat $($(, $rest)*)?>($($arg),*)
            }
            TableDataType::Text | TableDataType::Enum => $func::<String $($(, $rest)*)?>($($arg),*),
            TableDataType::Boolean => $func::<bool $($(, $rest)*)?>($($arg),*),
            TableDataType::Presence => $func::<() $($(, $rest)*)?>($($arg),*),
        }
    }};
}

// ---------------------------------------------------------------------------
// 浮動小数の Ord ラッパー
// ---------------------------------------------------------------------------

/// 浮動小数（f64）を `Ord` にするラッパー。
///
/// [`CellValue`] は `Ord` を要求するが浮動小数は `PartialOrd` しか持たないため、
/// `total_cmp` による全順序を与える（NaN でも panic しない）。
///
/// # `ordered-float` クレートを使わない理由
/// 全順序を与えるだけなら `ordered_float::OrderedFloat` で足りるが、この型には
/// 加えて `From<u16>` / `From<u32>`（`zoomOut` の平均・`falloffLinear*` が要求）と
/// `kasane_logic::merge_policy::saturating_add::Add` の実装が要る。
/// いずれも「外部の型に外部のトレイト」となり orphan rule で実装できないため、
/// ローカルの newtype が必須。`total_cmp` の再実装は 3 行なので依存を足す利点も無い。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OrderedFloat(pub f64);

impl Eq for OrderedFloat {}
impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}
impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl core::ops::Mul for OrderedFloat {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        OrderedFloat(self.0 * rhs.0)
    }
}
impl core::ops::Div for OrderedFloat {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        OrderedFloat(self.0 / rhs.0)
    }
}
impl core::ops::Sub for OrderedFloat {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        OrderedFloat(self.0 - rhs.0)
    }
}
impl kasane_logic::merge_policy::saturating_add::Add for OrderedFloat {
    fn saturating_add(self, rhs: Self) -> Self {
        OrderedFloat(self.0 + rhs.0)
    }
}
impl From<u16> for OrderedFloat {
    fn from(v: u16) -> Self {
        OrderedFloat(v as f64)
    }
}
// `falloffLinear*` は `TryFrom<u32>` を要求するが、浮動小数への変換は必ず成功する。
// `From` を実装すれば標準のブランケット実装経由で `TryFrom` も満たされる。
impl From<u32> for OrderedFloat {
    fn from(v: u32) -> Self {
        OrderedFloat(v as f64)
    }
}

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
pub trait Value: CellValue + Ord + 'static {
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
}

impl Value for OrderedFloat {
    fn type_name() -> &'static str {
        "Float"
    }

    fn accepts(data_type: TableDataType) -> bool {
        data_type == TableDataType::Float
    }

    fn decoder(_constraints: Option<&TableConstraints>) -> Result<Decoder<Self>, AppError> {
        Ok(Arc::new(|bytes: &[u8]| {
            <[u8; 8]>::try_from(bytes)
                .ok()
                .map(|b| OrderedFloat(f64::from_be_bytes(b)))
        }))
    }

    fn encode(&self, constraints: Option<&TableConstraints>) -> Result<Vec<u8>, AppError> {
        if let Some(TableConstraints::Float { min, max }) = constraints {
            check_range(&self.0, *min, *max)?;
        }
        Ok(self.0.to_be_bytes().to_vec())
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::Number::from_f64(self.0)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, AppError> {
        let v = value
            .as_f64()
            .ok_or_else(|| AppError::NumericValueOutOfRange {
                actual: value.to_string(),
                expected: "Float".to_string(),
            })?;
        if v.is_finite() {
            // `Ord` は `total_cmp` なので `-0.0 < 0.0` となり、両者は別の値として振る舞う。
            // 格納・フィルタ境界・`mapValues` の対応表がいずれもこの経路を通るので、
            // ここで `0.0` へ寄せておけば「`-0.0` を書いたのに一致しない」が起きない。
            Ok(OrderedFloat(if v == 0.0 { 0.0 } else { v }))
        } else {
            Err(AppError::NumericValueOutOfRange {
                actual: value.to_string(),
                expected: "finite Float".to_string(),
            })
        }
    }

    impl_ops!(OrderedFloat, dispatch_full);
    impl_falloff!(OrderedFloat, dispatch_full);
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
