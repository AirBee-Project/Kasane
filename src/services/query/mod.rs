//! `/query` の実行。
//!
//! Kasane 側の DSL（[`QueryNode`]）を Kasane-Logic の AST（`Query`）へ翻訳し、
//! 最適化と実行は Kasane-Logic に委ねる。入力源はテーブルごとの
//! [`TableSource`] で、対象領域に必要な範囲だけを LMDB から読む（遅延評価）。

pub mod value;

use std::collections::{BTreeMap, HashMap};

use kasane_logic::{Query, RangeId, Source};

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
type QuerySnapshot = <crate::backend::Db as Storage>::QuerySnapshot;

/// AST が参照する全テーブルを解決し、同時に読み取り権限を検査する。
///
/// 解決と認可を 1 回の読み取りにまとめてあるのが要点。認可（`check_tables`）が
/// `(db_id, table_id)` を引き、その直後にここが同じキーからメタデータを引く――という
/// 二重取得をなくすため。TiKV のように名前の解決が 1 件ずつ往復になるバックエンドでは、
/// 参照テーブル数に比例した往復が丸ごと 1 組ぶん消える。
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
    let resolved = app_state
        .db
        .read(async move |r| r.resolve_tables(&refs).await)
        .await?;

    // 認可は**存在の判定より先**に全件分行う。順序を逆にすると、権限の無い利用者へ
    // 404 で名前の存在有無を教えることになる（`authorize_resolved` の注記を参照）。
    for ((db_name, table_name), entry) in requested.iter().zip(&resolved) {
        crate::middleware::auth::authorize_resolved(
            user,
            entry.db_id,
            entry.table.as_ref().map(|t| t.id),
            db_name,
            Some(table_name),
            UserRole::Read,
        )?;
    }

    requested
        .into_iter()
        .zip(resolved)
        .map(|((db_name, table_name), entry)| {
            let table = entry.table.ok_or_else(|| AppError::TableNotFound {
                name: format!("{db_name}.{table_name}"),
            })?;
            Ok(((db_name, table_name), table))
        })
        .collect()
}

/// テーブルの格納値をクエリの値型 `V` へ復元するデコーダを組み立てる。
///
/// そのテーブルの `data_type` は `V` として読めなければならない
/// （`Text` と `Enum` はどちらも文字列として読める）。
fn build_decoder<V: Value>(table: &Table) -> Result<DecodeFn<V>, AppError> {
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
                // `output_type` は宣言だけで終わらせず、実際に組み立てる値型 `V` と
                // 一致することを検証する。多くの場合 `V` はリクエストの `value_type`
                // など、この式より外側で決まった型がそのまま流れてくるため、
                // `output_type` が実際には無視される余地がある。
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

/// `MapValues` を、入力側 `U` から出力側 `V` への写像として組み立てる。
///
/// 対応表は JSON リテラルなので、ここで一度だけ `U`/`V` へ解釈しておき、
/// FlexId ごとの評価では引き当てるだけにする。
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

/// クエリを実行し、対象空間IDの値を返す。
#[tracing::instrument(skip_all)]
pub async fn execute(
    app_state: &AppState,
    user: &User,
    request: ExecuteQueryRequest,
    query_params: &GetDataQuery,
) -> Result<GetDataResponse, AppError> {
    // テーブルの解決はトランザクション境界を跨ぐのでここで済ませ、
    // そのあとの評価（同期のクエリ実行器）だけをブロッキングタスクへ渡す。
    // 読み取り権限の検査もこの中で行う（`resolve_tables` の注記を参照）。
    let tables = resolve_tables(app_state, user, &request.query).await?;

    // このクエリが読む断面をここで 1 つに固定し、全ソースへ配る。
    // ソースごと・領域ごとに取り直すと、途中の書き込みを一部だけ見た結果が混ざる。
    let snapshot = app_state.db.query_snapshot().await?;

    let app_state = app_state.clone();
    let format = query_params.format;
    let limit = query_params.limit;

    // クエリ演算は同期ブロッキング処理のため、async ワーカーを塞がない。
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || -> Result<GetDataResponse, AppError> {
        span.in_scope(|| {
            let value_type = request
                .query
                .resolve_value_type(&tables, request.value_type)?;

            // 作業木は単一の値型で組まれるため、ここで値型ごとに単型化する。
            // Enum は格納こそ ID だが、クエリ上は選択肢の文字列（String）として扱う。
            for_value_type!(
                value_type, run, &app_state, &request, &tables, &snapshot, format, limit
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
) -> Result<GetDataResponse, AppError> {
    let targets = to_spatial_id_set(&request.spatial_ids)?;

    // 要求された空間IDそのものを評価境界にする。
    //
    // 外接矩形1つにまとめる手もあるが、`bounding_box()` は全 FlexId を覆う保証が無く
    // 取りこぼしが起きる。個々の領域を渡せば正確で、無関係な領域を読まずに済む。
    let bounds: Vec<RangeId> = targets.iter().map(|id| RangeId::from(&id)).collect();
    if bounds.is_empty() {
        // 対象領域が空。クエリを走らせるまでもない。
        let empty: Vec<(V, Vec<kasane_logic::FlexId>)> = Vec::new();
        return data_response::build(empty, format, limit, |v| Ok(v.to_json()));
    }

    // --- フェーズ 1: DSL → kasane-logic AST の翻訳 ---
    let ast = tracing::info_span!("query.translate", source_tables = tables.len())
        .in_scope(|| request.query.translate::<V>(app_state, tables, snapshot))?;

    tracing::debug!(
        "Executing query over {} source table(s), {} target region(s)",
        tables.len(),
        bounds.len()
    );

    // --- フェーズ 2: クエリ最適化 ---
    let optimized = tracing::info_span!("query.optimize").in_scope(|| ast.optimize());

    // --- フェーズ 3: 評価実行（最も重い処理）---
    // 対象領域をまとめて1回だけ評価し、結果を要求空間IDで絞る。
    // 空間ID1件ずつ `get()` を回すとクエリが件数分再実行されてしまう。
    let flex_ids = tracing::info_span!("query.run_within", target_regions = bounds.len())
        .in_scope(|| optimized.run_within(bounds).map_err(AppError::LogicError))?
        .into_iter()
        .filter(|(flex_id, _)| targets.get(flex_id).next().is_some());

    let by_value = group_by_value(flex_ids, limit);
    data_response::build(by_value, format, limit, |v| Ok(v.to_json()))
}

/// FlexId 列を値ごとにグループ化する（レスポンスは値辞書 + 空間ID群の形）。
///
/// `limit` が指定されている場合、保持する FlexId を `limit` 件までに抑える。
/// 本来は値の昇順に並べて上位 `limit` 件を返すべきだが、高速化のため順序を問わず
/// `limit` 件に達した時点で短絡評価（打ち切り）を行う。
///
/// `singleId` 形式では 1 つの `FlexId` が複数 FlexId へ展開されるので、`limit` 件の
/// `FlexId` を残せば出力は必ず `limit` 件以上に届く（不足しない）。
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
