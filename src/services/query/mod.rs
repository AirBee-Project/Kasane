//! `/query` の実行。
//!
//! Kasane 側の DSL（[`QueryNode`]）を Kasane-Logic の AST（`Query`）へ翻訳し、
//! 最適化と実行は Kasane-Logic に委ねる。入力源はテーブルごとの
//! [`TableSource`] で、対象領域に必要な範囲だけを LMDB から読む（遅延評価）。

pub mod value;

use std::collections::{BTreeMap, HashMap};

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
        query::{ExecuteQueryRequest, FilterCondition, MappingEntry, QueryNode},
    },
    repositories::database::table::data::query_source::TableSource,
    services::helpers::{data_response, spatial_ids::to_spatial_id_set},
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

/// テーブルの格納値をクエリの値型 `V` へ復元するデコーダを組み立てる。
///
/// そのテーブルの `data_type` は `V` として読めなければならない
/// （`Text` と `Enum` はどちらも文字列として読める）。
fn build_decoder<V: Value>(table: &Table) -> Result<Decoder<V>, AppError> {
    if !V::accepts(table.data_type) {
        return Err(value::incompatible_source(table, V::type_name()));
    }
    V::decoder(table.constraints.as_ref())
}

impl QueryNode {
    /// クエリ結果の値型を推論（または明示指定から決定）する。
    ///
    /// 全ソースの `data_type` が一致していればそれを採用する。一致しない場合は、
    /// 同じ値型として読める組合せ（`Text` と `Enum`）であっても推論はしないので、
    /// `value_type` の明示を要求する。
    ///
    /// `MapValues` の出力型は対応表のリテラルだけで決まりソースに依存しないため、
    /// その部分木は走査せず、推論にも寄与しない（`sources()` は逆に、テーブル解決の
    /// ため `MapValues` の下も辿る）。結果として `MapValues` しか持たないクエリの
    /// 出力型は推論できず、`value_type` の明示が要る。
    fn resolve_value_type(
        &self,
        tables: &ResolvedTables,
        explicit: Option<TableDataType>,
    ) -> Result<TableDataType, AppError> {
        if let Some(explicit) = explicit {
            return Ok(explicit);
        }

        let mut inferred: Option<TableDataType> = None;
        let mut has_map_values = false;
        let mut stack = vec![self];

        while let Some(node) = stack.pop() {
            if matches!(node, QueryNode::MapValues { .. }) {
                has_map_values = true;
                continue;
            }
            stack.extend(node.children());

            let QueryNode::Source { database, table } = node else {
                continue;
            };
            let data_type = tables[&(database.clone(), table.clone())].data_type;
            match inferred {
                None => inferred = Some(data_type),
                Some(existing) if existing != data_type => {
                    return Err(AppError::ConstraintViolation {
                        reason: format!(
                            "sources have different data types ({existing:?} and {data_type:?}); specify `value_type` explicitly if they can be read as the same type"
                        ),
                    });
                }
                Some(_) => {}
            }
        }

        inferred.ok_or_else(|| AppError::ConstraintViolation {
            reason: if has_map_values {
                "cannot infer the query value type because it contains a mapValues operator; specify `value_type` explicitly"
            } else {
                "cannot infer the query value type; specify `value_type` explicitly"
            }
            .to_string(),
        })
    }

    /// Kasane の DSL を Kasane-Logic の AST へ翻訳する。
    fn translate<V: Value>(
        &self,
        app_state: &AppState,
        tables: &ResolvedTables,
    ) -> Result<ValueQuery<V>, AppError> {
        match self {
            QueryNode::Source { database, table } => {
                let meta = &tables[&(database.clone(), table.clone())];
                let decode = build_decoder::<V>(meta)?;
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

            QueryNode::MapValues {
                input,
                input_type,
                mapping,
                default,
            } => {
                // 入力側の型は出力型 `V` とは独立に決まる。
                let input_type = input.resolve_value_type(tables, *input_type)?;
                for_value_type!(
                    input_type,
                    build_map_values[V],
                    input.as_ref(),
                    app_state,
                    tables,
                    mapping,
                    default
                )
            }
        }
    }
}

/// `MapValues` を、入力側 `U` から出力側 `V` への写像として組み立てる。
///
/// 対応表は JSON リテラルなので、ここで一度だけ `U`/`V` へ解釈しておき、
/// セルごとの評価では引き当てるだけにする。
fn build_map_values<U: Value, V: Value>(
    input: &QueryNode,
    app_state: &AppState,
    tables: &ResolvedTables,
    mapping: &[MappingEntry],
    default: &serde_json::Value,
) -> Result<ValueQuery<V>, AppError> {
    let input = input.translate::<U>(app_state, tables)?;

    let mut lookup: BTreeMap<U, V> = BTreeMap::new();
    for entry in mapping {
        let from = U::from_json(&entry.from)?.normalize_for_map();
        let to = V::from_json(&entry.to).map_err(|e| AppError::ConstraintViolation {
            reason: format!(
                "mapValues mapping value could not be parsed as the inferred type {}: {}",
                V::type_name(),
                e
            ),
        })?;
        // 後勝ちで黙って上書きすると、発火しない対応表エントリに気づけない。
        if lookup.insert(from, to).is_some() {
            return Err(AppError::ConstraintViolation {
                reason: format!("duplicate mapping key: {}", entry.from),
            });
        }
    }
    let default = V::from_json(default).map_err(|e| AppError::ConstraintViolation {
        reason: format!(
            "mapValues default value could not be parsed as the inferred type {}: {}",
            V::type_name(),
            e
        ),
    })?;

    Ok(input.map_values(move |value| {
        lookup
            .get(&value.normalize_for_map())
            .unwrap_or(&default)
            .clone()
    }))
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
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || -> Result<GetDataResponse, AppError> {
        span.in_scope(|| {
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
    let targets = to_spatial_id_set(&request.spatial_ids)?;

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
    // 空間ID1件ずつ `get()` を回すとクエリが件数分再実行されてしまう。
    let optimized = ast.optimize();
    let cells = optimized
        .run_on_subset(bounds)
        .map_err(AppError::LogicError)?
        .into_iter()
        .filter(|(flex_id, _)| targets.get(flex_id).next().is_some());

    let by_value = group_by_value(cells, limit);
    data_response::build(by_value, format, limit, |v| Ok(v.to_json()))
}

/// セル列を値ごとにグループ化する（レスポンスは値辞書 + 空間ID群の形）。
///
/// `limit` が指定されている場合、保持するセルを `limit` 件までに抑える。
/// [`data_response::build`] は値の昇順（`BTreeMap` の順）にグループを出力して
/// 上限へ達した時点で打ち切るため、**値が大きい側から捨てれば出力は変わらない**。
/// `limit` を無視して全件積むと、`?limit=10` でも結果セル数分のメモリを確保してしまう。
///
/// `singleId` 形式では 1 つの `FlexId` が複数セルへ展開されるので、`limit` 件の
/// `FlexId` を残せば出力は必ず `limit` 件以上に届く（不足しない）。
fn group_by_value<V: Value>(
    cells: impl Iterator<Item = (kasane_logic::FlexId, V)>,
    limit: Option<usize>,
) -> BTreeMap<V, Vec<kasane_logic::FlexId>> {
    let mut by_value: BTreeMap<V, Vec<kasane_logic::FlexId>> = BTreeMap::new();
    let mut held = 0usize;

    for (flex_id, value) in cells {
        by_value.entry(value).or_default().push(flex_id);
        held += 1;

        let Some(limit) = limit else { continue };
        while held > limit {
            let mut largest = by_value
                .last_entry()
                .expect("held > limit >= 0 なのでグループは存在する");
            let group = largest.get_mut();
            let drop_n = (held - limit).min(group.len());
            group.truncate(group.len() - drop_n);
            held -= drop_n;
            if group.is_empty() {
                largest.remove();
            }
        }
    }

    by_value
}
