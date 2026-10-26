//! `/query` の実行。翻訳だけを担い、実行は spatial-id に委ねる。

pub mod value;

use std::collections::{BTreeMap, HashMap};

use spatial_id::{
    RangeId,
    query::{CancellationToken, Query, Source},
};

use crate::{
    Kasane,
    error::{AppError, Resource},
    for_value_type,
    models::{
        ValueLiteral,
        data::{GetDataQuery, GetDataResponse, OutputFormat},
        query::{ExecuteQueryRequest, FilterCondition, MappingEntry, QueryNode},
        table::{Table, TableDataType},
        users::{User, UserRole},
    },
    repositories::{ReadRepository, Storage},
    services::helpers::{data_response, spatial_ids::to_spatial_id_set},
};

use crate::repositories::traits::DecodeFn;
use value::Value;

/// 解決済みのテーブルメタデータ表（`(database, table)` -> `Table`）。
type ResolvedTables = HashMap<(String, String), Table>;

/// このクエリが読む断面。全ソースで共有する（[`Storage::query_snapshot`] を参照）。
type QuerySnapshot = <crate::repositories::Db as Storage>::QuerySnapshot;

/// AST が参照する全テーブルを解決し、同時に読み取り権限を検査する。
///
/// 1 回の読み取りにまとめるのは、認可とメタデータ取得が同じキーを引く二重取得をなくすため。
/// 名前の解決が 1 件ずつ往復になるバックエンドでは、参照テーブル数ぶんの往復が丸ごと消える。
async fn resolve_tables(
    kasane: &Kasane,
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
    let resolved = kasane
        .db
        .read(async move |r| {
            let resolved = r.resolve_tables(&refs).await?;
            // 逆順にすると、権限の無い利用者へ `NotFound` で名前の存在有無を教えることになる。
            for ((db_name, table_name), entry) in for_auth.iter().zip(&resolved) {
                crate::services::auth::authorize_resolved(
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

    fn translate<V: Value>(
        &self,
        kasane: &Kasane,
        tables: &ResolvedTables,
        snapshot: &QuerySnapshot,
    ) -> Result<Query<V>, AppError> {
        match self {
            Self::Source { database, table } => {
                let meta = &tables[&(database.clone(), table.clone())];
                let decode = build_decoder::<V>(meta)?;
                Ok(kasane
                    .db
                    .table_source::<V>(meta.id, decode, snapshot.clone())
                    .query())
            }

            Self::ShiftX { input, z, index } => Ok(input
                .translate::<V>(kasane, tables, snapshot)?
                .shift_x(*z, *index)),
            Self::ShiftY { input, z, index } => Ok(input
                .translate::<V>(kasane, tables, snapshot)?
                .shift_y(*z, *index)),
            Self::ShiftF { input, z, index } => Ok(input
                .translate::<V>(kasane, tables, snapshot)?
                .shift_f(*z, *index)),

            Self::ZoomOut { input, z, policy } => {
                V::zoom_out(input.translate::<V>(kasane, tables, snapshot)?, *z, *policy)
            }

            Self::ExtrudeX {
                input,
                z,
                start,
                end,
                policy,
            } => V::extrude_x(
                input.translate::<V>(kasane, tables, snapshot)?,
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
                input.translate::<V>(kasane, tables, snapshot)?,
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
                input.translate::<V>(kasane, tables, snapshot)?,
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
                input.translate::<V>(kasane, tables, snapshot)?,
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
                input.translate::<V>(kasane, tables, snapshot)?,
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
                input.translate::<V>(kasane, tables, snapshot)?,
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
                left.translate::<V>(kasane, tables, snapshot)?,
                right.translate::<V>(kasane, tables, snapshot)?,
                V::from_value(default)?,
                *policy,
            ),

            Self::Difference { left, right } => Ok(left
                .translate::<V>(kasane, tables, snapshot)?
                .difference(right.translate::<V>(kasane, tables, snapshot)?)),

            Self::Intersection { left, right } => Ok(left
                .translate::<V>(kasane, tables, snapshot)?
                .intersection(right.translate::<V>(kasane, tables, snapshot)?)),

            Self::FilterValues { input, condition } => {
                let q = input.translate::<V>(kasane, tables, snapshot)?;
                let parse = |v: &Option<ValueLiteral>| -> Result<Option<V>, AppError> {
                    v.as_ref().map(V::from_value).transpose()
                };
                Ok(match condition {
                    FilterCondition::Equals { value } => q.filter_eq(V::from_value(value)?),
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
                let q = input.translate::<V>(kasane, tables, snapshot)?;
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
                    kasane,
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
    kasane: &Kasane,
    tables: &ResolvedTables,
    snapshot: &QuerySnapshot,
    mapping: &[MappingEntry],
    default: &ValueLiteral,
) -> Result<Query<V>, AppError> {
    let input = input.translate::<U>(kasane, tables, snapshot)?;

    let mut lookup: BTreeMap<U, V> = BTreeMap::new();
    for entry in mapping {
        let from = U::from_value(&entry.from)?;
        let to = V::from_value(&entry.to).map_err(|e| AppError::ConstraintViolation {
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
    let default = V::from_value(default).map_err(|e| AppError::ConstraintViolation {
        reason: format!(
            "mapValues default value could not be parsed as the inferred type {}: {}",
            V::type_name(),
            e
        ),
    })?;

    Ok(input.map_values(move |value| lookup.get(&value).unwrap_or(&default).clone()))
}

/// `spawn_blocking` は `JoinHandle` を drop しても中の処理を止めないため、`execute` の async フレームが drop されたら（＝上流でキャンセル）これで `token` を代わりにキャンセルする。
#[cfg(feature = "server")]
struct CancelOnDrop(CancellationToken);

#[cfg(feature = "server")]
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[tracing::instrument(skip_all)]
pub async fn execute(
    kasane: &Kasane,
    user: &User,
    request: ExecuteQueryRequest,
    query_params: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    let tables = resolve_tables(kasane, user, &request.query).await?;

    // 領域ごとに取り直すと、途中の書き込みを一部だけ見た結果が混ざる。
    let snapshot = kasane.db.query_snapshot().await?;
    let format = query_params.format;

    let value_type = request
        .query
        .resolve_value_type(&tables, request.value_type)?;

    // blocking へ渡すクロージャは `'static` が要るので、server ではハンドルを複製して持たせる。
    #[cfg(feature = "server")]
    let kasane = kasane.clone();
    // 作業木は単一の値型で組まれるため、ここで値型ごとに単型化する。
    let evaluate = move |token: &CancellationToken| {
        for_value_type!(
            value_type, run, &kasane, &request, &tables, &snapshot, format, token
        )
    };

    // クエリ演算は同期処理なので、サーバーでは async ワーカーを塞がないよう blocking へ逃がす。
    #[cfg(feature = "server")]
    {
        let token = CancellationToken::new();
        let _cancel_on_drop = CancelOnDrop(token.clone());
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || span.in_scope(|| evaluate(&token)))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
    }
    #[cfg(not(feature = "server"))]
    evaluate(&CancellationToken::new())
}

fn run<V: Value>(
    kasane: &Kasane,
    request: &ExecuteQueryRequest,
    tables: &ResolvedTables,
    snapshot: &QuerySnapshot,
    format: OutputFormat,
    token: &CancellationToken,
) -> Result<GetDataResponse, AppError> {
    let targets = to_spatial_id_set(&request.spatial_ids)?;

    // 外接矩形 1 つにまとめると `bounding_box()` が全 FlexId を覆わず取りこぼす。
    let bounds: Vec<RangeId> = targets.iter().map(|id| RangeId::from(&id)).collect();
    if bounds.is_empty() {
        let empty: Vec<(V, Vec<spatial_id::FlexId>)> = Vec::new();
        return data_response::build(empty, format, |v| Ok(v.to_value()));
    }

    let query = tracing::info_span!("query.translate", source_tables = tables.len())
        .in_scope(|| request.query.translate::<V>(kasane, tables, snapshot))?;

    // まとめて 1 回だけ評価する。空間 ID 1 件ずつだとクエリが件数分再実行される。
    let flex_ids = tracing::info_span!("query.run_within", target_regions = bounds.len())
        .in_scope(|| {
            query
                .get_overlapping_cancellable(targets.iter().collect::<Vec<_>>(), token)
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::from)
        })?
        .into_iter();

    let by_value = group_by_value(flex_ids);
    data_response::build(by_value, format, |v| Ok(v.to_value()))
}

fn group_by_value<V: Value>(
    flex_ids: impl Iterator<Item = (spatial_id::FlexId, V)>,
) -> BTreeMap<V, Vec<spatial_id::FlexId>> {
    let mut by_value: BTreeMap<V, Vec<spatial_id::FlexId>> = BTreeMap::new();

    for (flex_id, value) in flex_ids {
        by_value.entry(value).or_default().push(flex_id);
    }

    by_value
}
