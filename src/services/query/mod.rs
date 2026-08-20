//! `/query` の実行。翻訳だけを担い、最適化と実行は Kasane-Logic に委ねる。

pub mod value;

use std::collections::{BTreeMap, HashMap};

use kasane_logic::{Query, RangeId, Source};

use crate::{
    AppState,
    error::{AppError, Resource},
    for_value_type,
    models::{
        database::table::{
            Table, TableDataType,
            data::{GetDataQuery, OutputFormat},
        },
        query::{ExecuteQueryRequest, FilterCondition, MappingEntry, QueryNode},
        users::{User, UserRole},
    },
    repositories::{ReadRepository, Storage},
    services::helpers::spatial_ids::to_spatial_id_set,
};

use crate::repositories::traits::DecodeFn;
use value::Value;

/// 解決済みのテーブルメタデータ表（`(database, table)` -> `Table`）。
type ResolvedTables = HashMap<(String, String), Table>;

/// このクエリが読む断面。全ソースで共有する（[`Storage::query_snapshot`] を参照）。
type QuerySnapshot = <crate::backend::Db as Storage>::QuerySnapshot;

/// AST が参照する全テーブルを解決し、同時に読み取り権限を検査する。
///
/// 1 回の読み取りにまとめるのは、認可とメタデータ取得が同じキーを引く二重取得をなくすため。
/// 名前の解決が 1 件ずつ往復になるバックエンドでは、参照テーブル数ぶんの往復が丸ごと消える。
async fn resolve_tables(
    app_state: &AppState,
    user: &User,
    node: &QueryNode,
) -> Result<ResolvedTables, AppError> {
    let sources = node.sources();
    if sources.is_empty() {
        return Err(AppError::ConstraintViolation {
            reason: "query must reference at least one source table".to_string(),
        });
    }

    // 同じテーブルを複数箇所で参照するクエリはよくある。重複を落としてから引く。
    let mut refs: Vec<(String, String)> = sources
        .iter()
        .map(|(db_name, table_name)| (db_name.to_string(), table_name.to_string()))
        .collect();
    refs.sort_unstable();
    refs.dedup();

    let requested = refs.clone();
    let user = user.clone();
    let for_auth = requested.clone();
    // 解決と認可を同じ断面で行う。分けると、認可を通した対象と読む対象がずれうる。
    let resolved = app_state
        .db
        .read(async move |r| {
            let resolved = r.resolve_tables(&refs).await?;
            // 逆順にすると、権限の無い利用者へ 404 で名前の存在有無を教えることになる。
            for ((db_name, table_name), entry) in for_auth.iter().zip(&resolved) {
                crate::middleware::auth::authorize_resolved(
                    r,
                    &user,
                    entry.db_id,
                    entry.table.as_ref().map(|t| t.id),
                    db_name,
                    Some(table_name),
                    UserRole::Read,
                )
                .await?;
            }
            Ok(resolved)
        })
        .await?;

    requested
        .into_iter()
        .zip(resolved)
        .map(|((db_name, table_name), entry)| {
            let table = entry
                .table
                .ok_or_else(|| Resource::Table.not_found(format!("{db_name}.{table_name}")))?;
            Ok(((db_name, table_name), table))
        })
        .collect()
}

/// そのテーブルの `data_type` は `V` として読める必要がある（`Text` と `Enum` は同じ）。
fn build_decoder<V: Value>(table: &Table) -> Result<DecodeFn<V>, AppError> {
    if !V::accepts(table.data_type) {
        return Err(value::incompatible_source(table, V::type_name()));
    }
    V::decoder(table.constraints.as_ref())
}

impl QueryNode {
    /// クエリ結果の値型を推論（または明示指定から決定）する。
    ///
    /// 全ソースの `data_type` が一致していればそれを採用する。同じ値型として読める組合せ
    /// （`Text` と `Enum`）でも推論はせず、`value_type` の明示を要求する。
    fn resolve_value_type(
        &self,
        tables: &ResolvedTables,
        explicit: Option<TableDataType>,
    ) -> Result<TableDataType, AppError> {
        if let Some(explicit) = explicit {
            return Ok(explicit);
        }

        let mut inferred: Option<TableDataType> = None;
        let mut stack = vec![self];

        while let Some(node) = stack.pop() {
            let data_type = match node {
                Self::Source { database, table } => {
                    tables[&(database.clone(), table.clone())].data_type
                }
                Self::MapValues { output_type, .. } => *output_type,
                _ => {
                    stack.extend(node.children());
                    continue;
                }
            };

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
            reason: "cannot infer the query value type; specify `value_type` explicitly"
                .to_string(),
        })
    }

    /// Kasane の DSL を Kasane-Logic の AST へ翻訳する。
    fn translate<V: Value>(
        &self,
        app_state: &AppState,
        tables: &ResolvedTables,
        snapshot: &QuerySnapshot,
    ) -> Result<Query<V>, AppError> {
        match self {
            Self::Source { database, table } => {
                let meta = &tables[&(database.clone(), table.clone())];
                let decode = build_decoder::<V>(meta)?;
                Ok(app_state
                    .db
                    .table_source::<V>(meta.id, decode, snapshot.clone())
                    .query())
            }

            Self::ShiftX { input, z, index } => Ok(input
                .translate::<V>(app_state, tables, snapshot)?
                .shift_x(*z, *index)),
            Self::ShiftY { input, z, index } => Ok(input
                .translate::<V>(app_state, tables, snapshot)?
                .shift_y(*z, *index)),
            Self::ShiftF { input, z, index } => Ok(input
                .translate::<V>(app_state, tables, snapshot)?
                .shift_f(*z, *index)),

            Self::ZoomOut { input, z, policy } => V::zoom_out(
                input.translate::<V>(app_state, tables, snapshot)?,
                *z,
                *policy,
            ),

            Self::ExtrudeX {
                input,
                z,
                start,
                end,
                policy,
            } => V::extrude_x(
                input.translate::<V>(app_state, tables, snapshot)?,
                *z,
                *start,
                *end,
                *policy,
            ),
            Self::ExtrudeY {
                input,
                z,
                start,
                end,
                policy,
            } => V::extrude_y(
                input.translate::<V>(app_state, tables, snapshot)?,
                *z,
                *start,
                *end,
                *policy,
            ),
            Self::ExtrudeF {
                input,
                z,
                start,
                end,
                policy,
            } => V::extrude_f(
                input.translate::<V>(app_state, tables, snapshot)?,
                *z,
                *start,
                *end,
                *policy,
            ),

            Self::FalloffX {
                input,
                z,
                radius,
                pattern,
                direction,
                policy,
            } => V::falloff_x(
                input.translate::<V>(app_state, tables, snapshot)?,
                *z,
                *radius,
                direction.map(Into::into),
                (*pattern).into(),
                *policy,
            ),
            Self::FalloffY {
                input,
                z,
                radius,
                pattern,
                direction,
                policy,
            } => V::falloff_y(
                input.translate::<V>(app_state, tables, snapshot)?,
                *z,
                *radius,
                direction.map(Into::into),
                (*pattern).into(),
                *policy,
            ),
            Self::FalloffF {
                input,
                z,
                radius,
                pattern,
                direction,
                policy,
            } => V::falloff_f(
                input.translate::<V>(app_state, tables, snapshot)?,
                *z,
                *radius,
                direction.map(Into::into),
                (*pattern).into(),
                *policy,
            ),

            Self::Merge {
                left,
                right,
                default,
                policy,
            } => V::merge(
                left.translate::<V>(app_state, tables, snapshot)?,
                right.translate::<V>(app_state, tables, snapshot)?,
                V::from_json(default)?,
                *policy,
            ),

            Self::FilterValues { input, condition } => {
                let q = input.translate::<V>(app_state, tables, snapshot)?;
                let parse = |v: &Option<serde_json::Value>| -> Result<Option<V>, AppError> {
                    v.as_ref().map(V::from_json).transpose()
                };
                Ok(match condition {
                    FilterCondition::Equals { value } => q.filter_eq(V::from_json(value)?),
                    FilterCondition::InRange { min, max } => {
                        let start = parse(min)?
                            .map_or(core::ops::Bound::Unbounded, core::ops::Bound::Included);
                        let end = parse(max)?
                            .map_or(core::ops::Bound::Unbounded, core::ops::Bound::Included);
                        q.filter_in((start, end))
                    }
                    FilterCondition::NotInRange { min, max } => {
                        let start = parse(min)?
                            .map_or(core::ops::Bound::Unbounded, core::ops::Bound::Included);
                        let end = parse(max)?
                            .map_or(core::ops::Bound::Unbounded, core::ops::Bound::Included);
                        q.filter_not_in((start, end))
                    }
                })
            }

            Self::MathValues {
                input,
                operator,
                operand,
            } => {
                let q = input.translate::<V>(app_state, tables, snapshot)?;
                V::apply_math(q, *operator, *operand)
            }

            Self::MapValues {
                input,
                output_type,
                mapping,
                default,
            } => {
                // 検証しないと `output_type` が黙って無視されうる。
                if !V::accepts(*output_type) {
                    return Err(AppError::ConstraintViolation {
                        reason: format!(
                            "mapValues output_type {output_type:?} does not match the query value type {}",
                            V::type_name()
                        ),
                    });
                }

                // 入力側の型は常に入力元の部分木から推論する。
                let input_type = input.resolve_value_type(tables, None)?;
                for_value_type!(
                    input_type,
                    build_map_values[V],
                    input.as_ref(),
                    app_state,
                    tables,
                    snapshot,
                    mapping,
                    default
                )
            }
        }
    }
}

/// 対応表を一度だけ `U`/`V` へ解釈しておき、FlexId ごとの評価では引き当てるだけにする。
fn build_map_values<U: Value, V: Value>(
    input: &QueryNode,
    app_state: &AppState,
    tables: &ResolvedTables,
    snapshot: &QuerySnapshot,
    mapping: &[MappingEntry],
    default: &serde_json::Value,
) -> Result<Query<V>, AppError> {
    let input = input.translate::<U>(app_state, tables, snapshot)?;

    let mut lookup: BTreeMap<U, V> = BTreeMap::new();
    for entry in mapping {
        let from = U::from_json(&entry.from)?;
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

    Ok(input.map_values(move |value| lookup.get(&value).unwrap_or(&default).clone()))
}

/// `spawn_blocking` は `JoinHandle` を drop しても中の処理を止めないため、`execute` の async フレームが drop されたら（＝上流でキャンセル）これで `token` を代わりにキャンセルする。
struct CancelOnDrop(kasane_logic::CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// クエリを実行し、対象空間IDの値を返す。
#[tracing::instrument(skip_all)]
pub async fn execute(
    app_state: &AppState,
    user: &User,
    request: ExecuteQueryRequest,
    query_params: &GetDataQuery,
    is_arrow: bool,
) -> Result<axum::response::Response, AppError> {
    let tables = resolve_tables(app_state, user, &request.query).await?;
    let snapshot = app_state.db.query_snapshot().await?;

    let app_state = app_state.clone();
    let format = query_params.format;
    let limit = query_params.limit;

    let _permit = app_state
        .query_concurrency
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    let token = kasane_logic::CancellationToken::new();
    let _cancel_on_drop = CancelOnDrop(token.clone());

    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || -> Result<axum::response::Response, AppError> {
        let _permit = _permit;
        span.in_scope(|| {
            let value_type = request
                .query
                .resolve_value_type(&tables, request.value_type)?;

            for_value_type!(
                value_type, run, &app_state, &request, &tables, &snapshot, format, limit, is_arrow,
                &token, value_type
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
    snapshot: &QuerySnapshot,
    format: OutputFormat,
    limit: Option<usize>,
    is_arrow: bool,
    token: &kasane_logic::CancellationToken,
    value_type: TableDataType,
) -> Result<axum::response::Response, AppError> {
    let targets = to_spatial_id_set(&request.spatial_ids)?;
    let bounds: Vec<RangeId> = targets
        .range_ids_in(kasane_logic::AllowedIntervals::calendar())
        .collect();

    if bounds.is_empty() {
        let empty: Vec<(V, Vec<kasane_logic::FlexId>)> = Vec::new();
        return crate::services::helpers::stream_response::respond(
            empty,
            format,
            limit,
            value_type,
            is_arrow,
            |v| Ok(v.to_json()),
        );
    }

    let ast = tracing::info_span!("query.translate", source_tables = tables.len())
        .in_scope(|| request.query.translate::<V>(app_state, tables, snapshot))?;

    let optimized = tracing::info_span!("query.optimize").in_scope(|| ast.optimize());

    let flex_ids = tracing::info_span!("query.run_within", target_regions = bounds.len())
        .in_scope(|| {
            optimized
                .run_within(bounds, token)
                .map_err(AppError::LogicError)
        })?
        .into_iter()
        .filter(|(flex_id, _)| targets.get(flex_id).next().is_some());

    let by_value = group_by_value(flex_ids, limit);
    let groups: Vec<(V, Vec<kasane_logic::FlexId>)> = by_value.into_iter().collect();

    crate::services::helpers::stream_response::respond(
        groups,
        format,
        limit,
        value_type,
        is_arrow,
        |v| Ok(v.to_json()),
    )
}

/// FlexId 列を値ごとにグループ化する。
///
/// `limit` は値の昇順で上位を返すのが本来だが、高速化のため順序を問わず打ち切る。
/// `singleId` 形式では 1 つの `FlexId` が複数へ展開されるので、出力が不足することはない。
fn group_by_value<V: Value>(
    flex_ids: impl Iterator<Item = (kasane_logic::FlexId, V)>,
    limit: Option<usize>,
) -> BTreeMap<V, Vec<kasane_logic::FlexId>> {
    let mut by_value: BTreeMap<V, Vec<kasane_logic::FlexId>> = BTreeMap::new();
    let mut held = 0usize;

    for (flex_id, value) in flex_ids {
        by_value.entry(value).or_default().push(flex_id);
        held += 1;

        let Some(limit) = limit else { continue };
        if held >= limit {
            break;
        }
    }

    by_value
}
