//! `falloff_*` 単体のベンチ（実データ、半径を振る）。
//!
//! Kasane-Logic 側の `benches/query/unary/falloff.rs` と同じ半径の刻みで、
//! `services::query::execute` 経由（LMDB からの読み出し・レスポンス整形込み）の
//! コストを測る。半径ごとの伸びを Kasane-Logic 側の対応するベンチと突き合わせれば、
//! falloff 本体（Kasane-Logic）のコストと、その外側（Kasane）のコストのどちらが
//! 半径に対して支配的かが分かる。

use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};

use kasane::models::database::table::TableDataType;
use kasane::models::query::{ExecuteQueryRequest, FalloffPattern, MergePolicyKind, QueryNode};
use kasane::services::query;

#[path = "../../support/mod.rs"]
mod support;

const TABLE_NAME: &str = "risk_falloff";
const MAX_ZOOM_LEVEL: u8 = 25;
const QUERY_Z: u8 = 24;

fn falloff_query(field: &str, radius: u32) -> QueryNode {
    let input = Box::new(QueryNode::Source {
        database: support::DB_NAME.to_string(),
        table: TABLE_NAME.to_string(),
    });
    let (pattern, policy) = (FalloffPattern::Linear, MergePolicyKind::Max);
    match field {
        "x" => QueryNode::FalloffX {
            input,
            z: QUERY_Z,
            radius,
            pattern,
            direction: None,
            policy,
        },
        "y" => QueryNode::FalloffY {
            input,
            z: QUERY_Z,
            radius,
            pattern,
            direction: None,
            policy,
        },
        "f" => QueryNode::FalloffF {
            input,
            z: QUERY_Z,
            radius,
            pattern,
            direction: None,
            policy,
        },
        _ => unreachable!(),
    }
}

fn bench_falloff(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let env = support::build_env(&rt);
    support::load_risk_table(&rt, &env, TABLE_NAME, MAX_ZOOM_LEVEL);

    let spatial_ids = support::risk_data().coverage.clone();
    let query_params = support::default_query_params();

    let mut group = c.benchmark_group("Unary/Falloff");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(1));

    let run = |b: &mut criterion::Bencher, field: &'static str, radius: u32| {
        b.iter_batched(
            || ExecuteQueryRequest {
                value_type: Some(TableDataType::Int),
                spatial_ids: spatial_ids.clone(),
                query: falloff_query(field, radius),
            },
            |request| {
                rt.block_on(query::execute(
                    &env.app_state,
                    &env.user,
                    request,
                    &query_params,
                ))
                .unwrap()
            },
            BatchSize::SmallInput,
        );
    };

    for &dist in &[1u32, 5, 10, 15] {
        group.bench_with_input(BenchmarkId::new("falloff_x", dist), &dist, |b, &d| {
            run(b, "x", d)
        });
    }
    for &dist in &[1u32, 5, 10, 15] {
        group.bench_with_input(BenchmarkId::new("falloff_y", dist), &dist, |b, &d| {
            run(b, "y", d)
        });
    }
    for &dist in &[1u32, 5, 10, 15] {
        group.bench_with_input(BenchmarkId::new("falloff_f", dist), &dist, |b, &d| {
            run(b, "f", d)
        });
    }

    group.finish();
}

criterion_group!(benches, bench_falloff);
criterion_main!(benches);
