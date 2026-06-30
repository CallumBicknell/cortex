use cortex_core::{EventBus, InMemoryEventBus};
use criterion::{criterion_group, criterion_main, Criterion};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Debug, Clone)]
struct BenchEvent(u64);

impl cortex_core::Event for BenchEvent {}

fn bench_event_bus(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus");
    // Create a runtime that will be used for all benchmarks in this group.
    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    // Create the event bus and a dummy handler.
    let bus = InMemoryEventBus::new();
    let handler = Box::new(move |_event: Arc<dyn cortex_core::Event>| {
        Box::pin(async move {
            // Simulate some minimal work: yield once.
            tokio::task::yield_now().await;
        }) as Pin<Box<dyn Future<Output = ()> + Send + Sync>>
    })
        as Box<
            dyn Fn(Arc<dyn cortex_core::Event>) -> Pin<Box<dyn Future<Output = ()> + Send + Sync>>
                + Send
                + Sync,
        >;
    bus.subscribe(handler);

    // Benchmark publishing and handling a single event.
    group.bench_function("publish_handle_single", |b| {
        b.iter_batched(
            || Arc::new(BenchEvent(0)),
            |event| {
                // Clone the event for each iteration (though it's cheap).
                let event_clone = event.clone();
                // Block on the publish future.
                let _ = rt.block_on(bus.publish(event_clone));
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_event_bus);
criterion_main!(benches);
