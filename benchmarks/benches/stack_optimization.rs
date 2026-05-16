//! Classification: infrastructure
//! Status: active
//! CI coverage: criterion-smoke.yml benchmark compile check (compile-only),
//!   pure-rust-ci.yml dispatch-only benchmark compile check (compile-only)
//!
//! Stack implementation and memory pooling micro-benchmarks. Compares
//! Vec-based stacks vs persistent stacks with structural sharing, plus
//! fork/merge patterns at varying depths.

use adze::pool::NodePool;
use adze_glr_core::stack::StackNode;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::Arc;

/// Benchmark comparing old Vec-based stacks vs new persistent stacks
fn benchmark_stack_implementations(c: &mut Criterion) {
    let mut group = c.benchmark_group("stack_implementations");

    // Test different stack depths
    for depth in &[10, 50, 100, 500] {
        // Old approach: Vec cloning
        group.bench_with_input(BenchmarkId::new("vec_clone", depth), depth, |b, &d| {
            let mut base_stack = Vec::with_capacity(d);
            for i in 0..d {
                base_stack.push(i as u16);
            }

            b.iter(|| {
                let mut forks = Vec::with_capacity(10);
                for _ in 0..10 {
                    forks.push(base_stack.clone());
                }
                black_box(forks)
            });
        });

        // New approach: Persistent stacks with structural sharing
        group.bench_with_input(
            BenchmarkId::new("persistent_stack", depth),
            depth,
            |b, &d| {
                let mut base_stack = StackNode::new();
                for i in 0..d {
                    base_stack.push(i as u16, None);
                }

                b.iter(|| {
                    let mut forks = Vec::with_capacity(10);
                    for _ in 0..10 {
                        forks.push(base_stack.fork());
                    }
                    black_box(forks)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory pooling vs direct allocation
fn benchmark_memory_pooling(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pooling");

    #[derive(Default)]
    struct TestNode {
        #[allow(dead_code)]
        data: [u64; 16], // Larger node to make allocation cost visible
    }

    // Direct Arc allocation
    group.bench_function("direct_allocation", |b| {
        b.iter(|| {
            let mut nodes = Vec::with_capacity(100);
            for _ in 0..100 {
                nodes.push(Arc::new(TestNode::default()));
            }
            // Simulate usage and dropping
            nodes.clear();
            black_box(nodes)
        });
    });

    // With pooling
    group.bench_function("with_pooling", |b| {
        let pool = NodePool::<TestNode>::with_capacity(100);

        b.iter(|| {
            let mut nodes = Vec::with_capacity(100);
            for _ in 0..100 {
                nodes.push(pool.get_or_default());
            }
            // Return to pool
            for node in nodes.drain(..) {
                pool.put(node);
            }
            black_box(nodes)
        });
    });

    group.finish();
}

/// Benchmark fork/merge patterns with optimizations
fn benchmark_fork_merge_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("fork_merge_patterns");

    // Simulate a parse that forks frequently
    group.bench_function("frequent_fork_pattern", |b| {
        b.iter(|| {
            let stack = StackNode::with_state(0);
            let mut active_stacks = vec![stack];

            // Simulate parsing with frequent forks
            for i in 0..50 {
                let mut new_stacks = Vec::new();

                for mut s in active_stacks {
                    s.push(i, None);

                    // Fork on ambiguity (30% chance)
                    if i % 10 < 3 {
                        new_stacks.push(s.fork());
                        new_stacks.push(s.fork());
                    } else {
                        new_stacks.push(s);
                    }
                }

                // Merge compatible stacks (simple heuristic)
                if new_stacks.len() > 10 {
                    new_stacks.truncate(10);
                }

                active_stacks = new_stacks;
            }

            black_box(active_stacks)
        });
    });

    // Simulate a parse with deep recursion
    group.bench_function("deep_recursion_pattern", |b| {
        b.iter(|| {
            let mut stack = StackNode::new();

            // Push deeply
            for i in 0..200 {
                stack.push(i, None);
            }

            // Fork at various depths
            let mut forks = Vec::new();
            for _ in 0..5 {
                let mut fork = stack.fork();

                // Pop some items
                for _ in 0..50 {
                    fork.pop();
                }

                // Push new items
                for i in 150..180 {
                    fork.push(i, None);
                }

                forks.push(fork);
            }

            black_box(forks)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_stack_implementations,
    benchmark_memory_pooling,
    benchmark_fork_merge_patterns
);
criterion_main!(benches);
