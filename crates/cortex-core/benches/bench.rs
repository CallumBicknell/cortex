use criterion::{criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("fast", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
