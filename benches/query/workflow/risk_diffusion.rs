use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use kasane::models::database::table::TableDataType;
use kasane::models::query::{
    Direction, ExecuteQueryRequest, FalloffPattern, MergePolicyKind, QueryNode,
};
use kasane::services::query;

#[path = "../../support/mod.rs"]
mod support;

const TABLE_NAME: &str = "risk_diffusion";
const MAX_ZOOM_LEVEL: u8 = 25;

fn build_query() -> QueryNode {
    QueryNode::FalloffY {
        input: Box::new(QueryNode::FalloffX {
            input: Box::new(QueryNode::FalloffF {
                input: Box::new(QueryNode::ZoomOut {
                    input: Box::new(QueryNode::Source {
                        database: support::DB_NAME.to_string(),
                        table: TABLE_NAME.to_string(),
                    }),
                    z: 22,
                    policy: MergePolicyKind::Max,
                }),
                z: 25,
                radius: 10,
                pattern: FalloffPattern::Linear,
                direction: Some(Direction::Upper),
                policy: MergePolicyKind::Max,
            }),
            z: 25,
            radius: 10,
            pattern: FalloffPattern::Linear,
            direction: None,
            policy: MergePolicyKind::Max,
        }),
        z: 25,
        radius: 10,
        pattern: FalloffPattern::Linear,
        direction: None,
        policy: MergePolicyKind::Max,
    }
}

fn bench_risk_diffusion(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let env = support::build_env(&rt);
    support::load_risk_table(&rt, &env, TABLE_NAME, MAX_ZOOM_LEVEL);

    // データ全体を覆う領域を対象にする（Kasane-Logic 側の `raw_run()` が全木を
    // 評価するのと同じく、範囲での絞り込みをボトルネックに含めない）。
    let spatial_ids = support::risk_data().coverage.clone();
    let query_params = support::default_query_params();

    let mut group = c.benchmark_group("Workflow/RiskDiffusion");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(1));

    group.bench_function("execute_query", |b| {
        b.iter_batched(
            || ExecuteQueryRequest {
                value_type: Some(TableDataType::Int),
                spatial_ids: spatial_ids.clone(),
                query: build_query(),
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
    });

    group.finish();
}

criterion_group!(benches, bench_risk_diffusion);
criterion_main!(benches);
