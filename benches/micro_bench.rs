use criterion::{black_box, Criterion};

// ─── Benchmarks ────────────────────────────────────────────────────────────────

pub fn bench_config_load(c: &mut Criterion) {
    c.bench_function("config_default", |b| {
        b.iter(|| {
            let cfg = sparrow::config::Config::default();
            black_box(cfg);
        })
    });
}

pub fn bench_provider_registry(c: &mut Criterion) {
    c.bench_function("provider_registry", |b| {
        b.iter(|| {
            let providers = sparrow::config::providers::provider_registry();
            black_box(providers);
        })
    });
}

pub fn bench_redaction(c: &mut Criterion) {
    let filter = sparrow::redaction::RedactionFilter::new();
    let text = "Using key sk-ant-api03-abcdef123456789 for authentication with ghp_token_xyz";
    c.bench_function("redaction_filter", |b| {
        b.iter(|| {
            let result = filter.redact_str(black_box(text));
            black_box(result);
        })
    });
}

pub fn bench_event_serialization(c: &mut Criterion) {
    let event = sparrow::event::Event::RunStarted {
        run: sparrow::event::RunId("bench-1".into()),
        task: "benchmark task".into(),
        agent: "sparrow".into(),
    };
    c.bench_function("event_serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&event)).unwrap();
            black_box(json);
        })
    });
}

pub fn bench_skill_relevance(c: &mut Criterion) {
    let skill = sparrow::capabilities::Skill {
        name: "test".into(), description: "desc".into(),
        trigger: vec!["rust".into(), "error".into(), "handling".into()],
        body: "body".into(), source_file: "".into(),
        usage_count: 0, created_at: "".into(), score: 0.5, auto_generated: false,
    };
    let ctx = "I need help with Rust error handling in async code with anyhow";
    c.bench_function("skill_relevance", |b| {
        b.iter(|| {
            let score = skill.relevance(black_box(ctx));
            black_box(score);
        })
    });
}

pub fn bench_autonomy_decision(c: &mut Criterion) {
    let contract = sparrow::autonomy::AutonomyContract::supervised();
    let action = sparrow::autonomy::ProposedAction {
        tool_name: "edit".into(),
        risk: sparrow::event::RiskLevel::Mutating,
        args: serde_json::json!({"path": "src/main.rs"}),
    };
    c.bench_function("autonomy_decision", |b| {
        b.iter(|| {
            let decision = contract.decide(black_box(&action));
            black_box(decision);
        })
    });
}

pub fn bench_all(c: &mut Criterion) {
    bench_config_load(c);
    bench_provider_registry(c);
    bench_redaction(c);
    bench_event_serialization(c);
    bench_skill_relevance(c);
    bench_autonomy_decision(c);
}

criterion::criterion_group!(benches, bench_all);
criterion::criterion_main!(benches);
