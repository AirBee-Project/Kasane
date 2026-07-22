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
    models::{
        database::table::{
            Table, TableDataType,
            data::{GetDataQuery, GetDataResponse, OutputFormat},
        },
        query::{ExecuteQueryRequest, FilterMode, QueryNode, ValueConvert},
    },
    repositories::database::table::data::query_source::TableSource,
    services::helpers::{data_response, spatial_ids::process_spatial_ids, value::interpret_value},
};

use value::{Decoder, OrderedF32, OrderedF64, QueryValue, ValueQuery};

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

/// クエリ結果の値型を決める。
///
/// 明示指定があればそれを使う。無い場合は「変換表を持たない全ソースの `data_type` が
/// 一致していること」を条件に推論する。変換表を使うクエリでは推論できないため明示が必要。
fn resolve_value_type(
    request: &ExecuteQueryRequest,
    tables: &ResolvedTables,
) -> Result<TableDataType, AppError> {
    if let Some(explicit) = request.value_type {
        return Ok(explicit);
    }

    let mut converted_exists = false;
    let mut inferred: Option<TableDataType> = None;
    collect_source_types(&request.query, tables, &mut inferred, &mut converted_exists)?;

    match inferred {
        Some(t) if !converted_exists => Ok(t),
        _ => Err(AppError::ConstraintViolation {
            reason: "cannot infer the query value type; specify `value_type` explicitly \
                     (required when a source uses a conversion table)"
                .to_string(),
        }),
    }
}

/// 変換表を持たないソースの `data_type` が全て一致するかを調べる。
fn collect_source_types(
    node: &QueryNode,
    tables: &ResolvedTables,
    inferred: &mut Option<TableDataType>,
    converted_exists: &mut bool,
) -> Result<(), AppError> {
    match node {
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
            collect_source_types(left, tables, inferred, converted_exists)?;
            collect_source_types(right, tables, inferred, converted_exists)
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
            collect_source_types(input, tables, inferred, converted_exists)
        }
    }
}

/// 参照テーブルの `max_zoom_level` の最小値。対象領域の解決に使う。
fn min_zoom(tables: &ResolvedTables) -> u8 {
    tables
        .values()
        .map(|t| t.max_zoom_level)
        .min()
        .expect("resolve_tables rejects empty source sets")
}

/// テーブルの格納値をクエリの値型 `V` へ写すデコーダを組み立てる。
///
/// 変換表が無い場合、そのテーブルの `data_type` は `V` と一致していなければならない。
/// 変換表がある場合は、`from` を**そのテーブルの格納形式へエンコードした結果**を鍵にする
/// （保存時とまったく同じ符号化を使うので、型ごとの比較を書き分けずに済む）。
fn build_decoder<V: QueryValue>(
    table: &Table,
    convert: Option<&ValueConvert>,
) -> Result<Decoder<V>, AppError> {
    let Some(convert) = convert else {
        // 変換表が無い場合は、そのテーブルの格納形式から直接復元できなければエラー。
        return V::decoder(table);
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

/// Kasane の DSL を Kasane-Logic の AST へ翻訳する。
fn translate<V: QueryValue>(
    app_state: &AppState,
    node: &QueryNode,
    tables: &ResolvedTables,
) -> Result<ValueQuery<V>, AppError> {
    match node {
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
            Ok(translate::<V>(app_state, input, tables)?.shift_x(*z, *index))
        }
        QueryNode::ShiftY { input, z, index } => {
            Ok(translate::<V>(app_state, input, tables)?.shift_y(*z, *index))
        }
        QueryNode::ShiftF { input, z, index } => {
            Ok(translate::<V>(app_state, input, tables)?.shift_f(*z, *index))
        }

        QueryNode::ZoomOut { input, z, policy } => {
            V::zoom_out(translate::<V>(app_state, input, tables)?, *z, *policy)
        }

        QueryNode::ExtrudeX {
            input,
            z,
            start,
            end,
            policy,
        } => V::extrude_x(
            translate::<V>(app_state, input, tables)?,
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
            translate::<V>(app_state, input, tables)?,
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
            translate::<V>(app_state, input, tables)?,
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
            translate::<V>(app_state, input, tables)?,
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
            translate::<V>(app_state, input, tables)?,
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
            translate::<V>(app_state, input, tables)?,
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
            translate::<V>(app_state, left, tables)?,
            translate::<V>(app_state, right, tables)?,
            V::from_json(default)?,
            *policy,
        ),

        QueryNode::FilterValues {
            input,
            mode,
            value,
            min,
            max,
        } => {
            let q = translate::<V>(app_state, input, tables)?;
            let parse = |v: &Option<serde_json::Value>| -> Result<Option<V>, AppError> {
                v.as_ref().map(V::from_json).transpose()
            };
            Ok(match mode {
                FilterMode::Equals => {
                    let target = value
                        .as_ref()
                        .ok_or_else(|| AppError::ConstraintViolation {
                            reason: "filterValues with mode 'equals' requires `value`".to_string(),
                        })?;
                    q.retain_value_eq(V::from_json(target)?)
                }
                FilterMode::InRange => q.retain_value_in_range(parse(min)?, parse(max)?),
                FilterMode::NotInRange => q.retain_value_not_in_range(parse(min)?, parse(max)?),
            })
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
        let value_type = resolve_value_type(&request, &tables)?;

        // 作業木は単一の値型で組まれるため、ここで値型ごとに単型化する。
        match value_type {
            TableDataType::TinyInt => run::<i8>(&app_state, &request, &tables, format, limit),
            TableDataType::SmallInt => run::<i16>(&app_state, &request, &tables, format, limit),
            TableDataType::Int => run::<i32>(&app_state, &request, &tables, format, limit),
            TableDataType::BigInt => run::<i64>(&app_state, &request, &tables, format, limit),
            TableDataType::Float => run::<OrderedF32>(&app_state, &request, &tables, format, limit),
            TableDataType::Double => {
                run::<OrderedF64>(&app_state, &request, &tables, format, limit)
            }
            // Enum は格納こそ ID だが、クエリ上は選択肢の文字列として扱う。
            TableDataType::Text | TableDataType::Enum => {
                run::<String>(&app_state, &request, &tables, format, limit)
            }
            TableDataType::Boolean => run::<bool>(&app_state, &request, &tables, format, limit),
            TableDataType::Presence => run::<()>(&app_state, &request, &tables, format, limit),
        }
    })
    .await
    .map_err(|e| AppError::InternalError(e.to_string()))?
}

fn run<V: QueryValue>(
    app_state: &AppState,
    request: &ExecuteQueryRequest,
    tables: &ResolvedTables,
    format: OutputFormat,
    limit: Option<usize>,
) -> Result<GetDataResponse, AppError> {
    let targets = process_spatial_ids(
        &request.spatial_ids,
        min_zoom(tables),
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

    let ast = translate::<V>(app_state, &request.query, tables)?;
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
