//! `/query` の実行。
//!
//! Kasane 側の DSL（[`QueryNode`]）を Kasane-Logic の AST（`Query`）へ翻訳し、
//! 最適化と実行は Kasane-Logic に委ねる。入力源はテーブルごとの
//! [`TableSource`] で、対象領域に必要な範囲だけを LMDB から読む（遅延評価）。

pub mod value;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use kasane_logic::{RangeId, Source};

use crate::{
    AppState,
    error::AppError,
    for_value_type,
    models::{
        database::table::{
            Table, TableDataType,
            data::{GetDataQuery, GetDataResponse, OutputFormat},
        },
        query::{ExecuteQueryRequest, FilterCondition, QueryNode, ValueConvert},
    },
    repositories::database::table::data::query_source::TableSource,
    services::helpers::{data_response, spatial_ids::process_spatial_ids, value::interpret_value},
};

use value::{Decoder, Value, ValueQuery};

/// 解決済みのテーブルメタデータ表（`(database, table)` -> `Table`）。
type ResolvedTables = HashMap<(String, String), Table>;

/// AST が参照する全テーブルを解決する。
fn resolve_tables(app_state: &AppState, node: &QueryNode) -> Result<ResolvedTables, AppError> {
    let refs = node.sources();
    if refs.is_empty() {
        return Err(AppError::ConstraintViolation {
            reason: "query must reference at least one source table".to_string(),
        });
    }

    let mut tables: ResolvedTables = HashMap::new();
    app_state.db.read(|r| {
        for (db_name, table_name) in &refs {
            let key = (db_name.to_string(), table_name.to_string());
            if tables.contains_key(&key) {
                continue;
            }
            let table =
                r.table_info(db_name, table_name)?
                    .ok_or_else(|| AppError::TableNotFound {
                        name: format!("{db_name}.{table_name}"),
                    })?;
            tables.insert(key, table);
        }
        Ok(())
    })?;
    Ok(tables)
}

/// 参照テーブルの `max_zoom_level` の最大値（最も細かいテーブルの解像度）。
///
/// 対象空間IDの解像度上限に使う。最小値（最も粗いテーブル）に合わせると、細かいテーブルに
/// 本来入る空間IDまで `Error` で弾かれたり `Normalize` で粗く丸められたりして解像度が潰れる。
/// 最も細かい解像度を上限にすれば、粗いテーブルもその領域を内包するリーフの値として正しく
/// 寄与する（`descend_range` / `get_range`）ため、情報を落とさずに済む。
/// 単一テーブル参照時は search 系 API と同じくそのテーブルの `max_zoom_level` に一致する。
fn max_zoom(tables: &ResolvedTables) -> u8 {
    tables
        .values()
        .map(|t| t.max_zoom_level)
        .max()
        .expect("resolve_tables rejects empty source sets")
}

/// テーブルの格納値をクエリの値型 `V` へ写すデコーダを組み立てる。
///
/// 変換表が無い場合、そのテーブルの `data_type` は `V` と一致していなければならない。
/// 変換表がある場合は、`from` を**そのテーブルの格納形式へエンコードした結果**を鍵にする
/// （保存時とまったく同じ符号化を使うので、型ごとの比較を書き分けずに済む）。
fn build_decoder<V: Value>(
    table: &Table,
    convert: Option<&ValueConvert>,
) -> Result<Decoder<V>, AppError> {
    let Some(convert) = convert else {
        // 変換表が無い場合は、そのテーブルの格納形式が V と一致していなければならない。
        if !V::accepts(table.data_type) {
            return Err(value::incompatible_source(table, V::type_name()));
        }
        return V::decoder(table.constraints.as_ref());
    };

    let mut map: HashMap<Vec<u8>, V> = HashMap::with_capacity(convert.entries.len());
    for entry in &convert.entries {
        let key = interpret_value(
            table.data_type,
            table.constraints.as_ref(),
            entry.from.clone(),
        )?;
        map.insert(key, V::from_json(&entry.to)?);
    }
    let default = convert.default.as_ref().map(V::from_json).transpose()?;

    Ok(Arc::new(move |bytes: &[u8]| {
        map.get(bytes).cloned().or_else(|| default.clone())
    }))
}

/// ASTの翻訳とコンテキスト解決機能を提供する拡張トレイト。
pub trait QueryNodeExt {
    /// クエリ結果の値型を推論（または明示指定から決定）する。
    fn resolve_value_type(
        &self,
        tables: &ResolvedTables,
        explicit: Option<TableDataType>,
    ) -> Result<TableDataType, AppError>;

    /// 変換表を持たないソースの `data_type` が全て一致するかを調べる内部メソッド。
    fn collect_source_types(
        &self,
        tables: &ResolvedTables,
        inferred: &mut Option<TableDataType>,
        converted_exists: &mut bool,
    ) -> Result<(), AppError>;

    /// Kasane の DSL を Kasane-Logic の AST へ翻訳する。
    fn translate<V: Value>(
        &self,
        app_state: &AppState,
        tables: &ResolvedTables,
    ) -> Result<ValueQuery<V>, AppError>;
}

impl QueryNodeExt for QueryNode {
    fn resolve_value_type(
        &self,
        tables: &ResolvedTables,
        explicit: Option<TableDataType>,
    ) -> Result<TableDataType, AppError> {
        if let Some(explicit) = explicit {
            return Ok(explicit);
        }

        let mut converted_exists = false;
        let mut inferred: Option<TableDataType> = None;
        self.collect_source_types(tables, &mut inferred, &mut converted_exists)?;

        match inferred {
            Some(t) if !converted_exists => Ok(t),
            _ => Err(AppError::ConstraintViolation {
                reason: "cannot infer the query value type; specify `value_type` explicitly \
                         (required when a source uses a conversion table)"
                    .to_string(),
            }),
        }
    }

    fn collect_source_types(
        &self,
        tables: &ResolvedTables,
        inferred: &mut Option<TableDataType>,
        converted_exists: &mut bool,
    ) -> Result<(), AppError> {
        match self {
            QueryNode::Source {
                database,
                table,
                convert,
            } => {
                if convert.is_some() {
                    *converted_exists = true;
                    return Ok(());
                }
                let meta = &tables[&(database.clone(), table.clone())];
                match inferred {
                    None => *inferred = Some(meta.data_type),
                    Some(existing) if *existing != meta.data_type => {
                        return Err(AppError::ConstraintViolation {
                            reason: format!(
                                "sources have different data types ({:?} and {:?}); \
                                 specify `value_type` and give each source a conversion table",
                                existing, meta.data_type
                            ),
                        });
                    }
                    Some(_) => {}
                }
                Ok(())
            }
            QueryNode::Merge { left, right, .. } => {
                left.collect_source_types(tables, inferred, converted_exists)?;
                right.collect_source_types(tables, inferred, converted_exists)
            }
            QueryNode::ShiftX { input, .. }
            | QueryNode::ShiftY { input, .. }
            | QueryNode::ShiftF { input, .. }
            | QueryNode::ZoomOut { input, .. }
            | QueryNode::ExtrudeX { input, .. }
            | QueryNode::ExtrudeY { input, .. }
            | QueryNode::ExtrudeF { input, .. }
            | QueryNode::FalloffLinearX { input, .. }
            | QueryNode::FalloffLinearY { input, .. }
            | QueryNode::FalloffLinearF { input, .. }
            | QueryNode::FilterValues { input, .. } => {
                input.collect_source_types(tables, inferred, converted_exists)
            }
        }
    }

    fn translate<V: Value>(
        &self,
        app_state: &AppState,
        tables: &ResolvedTables,
    ) -> Result<ValueQuery<V>, AppError> {
        match self {
            QueryNode::Source {
                database,
                table,
                convert,
            } => {
                let meta = &tables[&(database.clone(), table.clone())];
                let decode = build_decoder::<V>(meta, convert.as_ref())?;
                Ok(TableSource::<V>::new(
                    app_state.db.env.clone(),
                    app_state.db.tables_data,
                    meta.id,
                    decode,
                )
                .query())
            }

            QueryNode::ShiftX { input, z, index } => {
                Ok(input.translate::<V>(app_state, tables)?.shift_x(*z, *index))
            }
            QueryNode::ShiftY { input, z, index } => {
                Ok(input.translate::<V>(app_state, tables)?.shift_y(*z, *index))
            }
            QueryNode::ShiftF { input, z, index } => {
                Ok(input.translate::<V>(app_state, tables)?.shift_f(*z, *index))
            }

            QueryNode::ZoomOut { input, z, policy } => {
                V::zoom_out(input.translate::<V>(app_state, tables)?, *z, *policy)
            }

            QueryNode::ExtrudeX {
                input,
                z,
                start,
                end,
                policy,
            } => V::extrude_x(
                input.translate::<V>(app_state, tables)?,
                *z,
                *start,
                *end,
                *policy,
            ),
            QueryNode::ExtrudeY {
                input,
                z,
                start,
                end,
                policy,
            } => V::extrude_y(
                input.translate::<V>(app_state, tables)?,
                *z,
                *start,
                *end,
                *policy,
            ),
            QueryNode::ExtrudeF {
                input,
                z,
                start,
                end,
                policy,
            } => V::extrude_f(
                input.translate::<V>(app_state, tables)?,
                *z,
                *start,
                *end,
                *policy,
            ),

            QueryNode::FalloffLinearX {
                input,
                z,
                radius,
                policy,
            } => V::falloff_x(
                input.translate::<V>(app_state, tables)?,
                *z,
                *radius,
                *policy,
            ),
            QueryNode::FalloffLinearY {
                input,
                z,
                radius,
                policy,
            } => V::falloff_y(
                input.translate::<V>(app_state, tables)?,
                *z,
                *radius,
                *policy,
            ),
            QueryNode::FalloffLinearF {
                input,
                z,
                radius,
                policy,
            } => V::falloff_f(
                input.translate::<V>(app_state, tables)?,
                *z,
                *radius,
                *policy,
            ),

            QueryNode::Merge {
                left,
                right,
                default,
                policy,
            } => V::merge(
                left.translate::<V>(app_state, tables)?,
                right.translate::<V>(app_state, tables)?,
                V::from_json(default)?,
                *policy,
            ),

            QueryNode::FilterValues { input, condition } => {
                let q = input.translate::<V>(app_state, tables)?;
                let parse = |v: &Option<serde_json::Value>| -> Result<Option<V>, AppError> {
                    v.as_ref().map(V::from_json).transpose()
                };
                Ok(match condition {
                    FilterCondition::Equals { value } => q.filter_eq(V::from_json(value)?),
                    FilterCondition::InRange { min, max } => {
                        let start = parse(min)?
                            .map(core::ops::Bound::Included)
                            .unwrap_or(core::ops::Bound::Unbounded);
                        let end = parse(max)?
                            .map(core::ops::Bound::Included)
                            .unwrap_or(core::ops::Bound::Unbounded);
                        q.filter_in((start, end))
                    }
                    FilterCondition::NotInRange { min, max } => {
                        let start = parse(min)?
                            .map(core::ops::Bound::Included)
                            .unwrap_or(core::ops::Bound::Unbounded);
                        let end = parse(max)?
                            .map(core::ops::Bound::Included)
                            .unwrap_or(core::ops::Bound::Unbounded);
                        q.filter_not_in((start, end))
                    }
                })
            }
        }
    }
}

/// クエリを実行し、対象空間IDの値を返す。
pub async fn execute(
    app_state: &AppState,
    request: ExecuteQueryRequest,
    query_params: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    let app_state = app_state.clone();
    let format = query_params.format;
    let limit = query_params.limit;

    // LMDB 読み取りと演算はいずれも同期ブロッキング処理のため、async ワーカーを塞がない。
    tokio::task::spawn_blocking(move || -> Result<GetDataResponse, AppError> {
        let tables = resolve_tables(&app_state, &request.query)?;
        let value_type = request
            .query
            .resolve_value_type(&tables, request.value_type)?;

        // 作業木は単一の値型で組まれるため、ここで値型ごとに単型化する。
        // Enum は格納こそ ID だが、クエリ上は選択肢の文字列（String）として扱う。
        for_value_type!(
            value_type, run, &app_state, &request, &tables, format, limit
        )
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

fn run<V: Value>(
    app_state: &AppState,
    request: &ExecuteQueryRequest,
    tables: &ResolvedTables,
    format: OutputFormat,
    limit: Option<usize>,
) -> Result<GetDataResponse, AppError> {
    let targets = process_spatial_ids(
        &request.spatial_ids,
        max_zoom(tables),
        &request.zoom_level_policy,
    )?;

    // 要求された空間IDそのものを評価境界にする。
    //
    // 外接矩形1つにまとめる手もあるが、`bounding_box()` は全セルを覆う保証が無く
    // 取りこぼしが起きる。個々の領域を渡せば正確で、無関係な領域を読まずに済む。
    let bounds: Vec<RangeId> = targets.iter().map(|id| RangeId::from(&id)).collect();
    if bounds.is_empty() {
        // 対象領域が空。クエリを走らせるまでもない。
        let empty: Vec<(V, Vec<kasane_logic::FlexId>)> = Vec::new();
        return data_response::build(empty, format, limit, |v| Ok(v.to_json()));
    }

    let ast = request.query.translate::<V>(app_state, tables)?;
    tracing::debug!(
        "Executing query over {} source table(s), {} target region(s)",
        tables.len(),
        bounds.len()
    );

    // 対象領域をまとめて1回だけ評価し、結果を要求空間IDで絞る。
    // 空間ID1件ずつ `lazy().get()` を回すとクエリが件数分再実行されてしまう。
    let optimized = ast.optimize();
    let cells = optimized
        .run_on_subset(bounds)
        .map_err(AppError::LogicError)?
        .into_iter()
        .filter(|(flex_id, _)| targets.get(flex_id).next().is_some());

    // 値ごとにグループ化する（レスポンスは値辞書 + 空間ID群の形）。
    let mut by_value: BTreeMap<V, Vec<kasane_logic::FlexId>> = BTreeMap::new();
    for (flex_id, value) in cells {
        by_value.entry(value).or_default().push(flex_id);
    }

    data_response::build(by_value, format, limit, |v| Ok(v.to_json()))
}
