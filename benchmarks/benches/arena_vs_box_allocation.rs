//! Classification: infrastructure
//! Status: active
//! CI coverage: criterion-smoke.yml benchmark compile check (compile-only),
//!   pure-rust-ci.yml dispatch-only benchmark compile check (compile-only)
//!
//! Arena vs Box Allocation Benchmark
//!
//! This benchmark compares the performance of arena allocation vs individual Box allocations
//! for parse tree nodes. It validates the ≥50% allocation count reduction and ≥20% speedup
//! targets from the v0.8.0 Performance Contract.
//!
//! Run with: cargo bench --bench arena_vs_box_allocation

use adze::arena_allocator::{TreeArena, TreeNode};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// Benchmark arena allocation for N nodes
fn bench_arena_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_allocation");

    for size in [100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut arena = TreeArena::new();
                for i in 0..size {
                    let handle = arena.alloc(TreeNode::leaf(i as i32));
                    black_box(handle);
                }
                black_box(arena);
            });
        });
    }
    group.finish();
}

/// Benchmark Box allocation for N nodes
fn bench_box_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("box_allocation");

    for size in [100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut nodes = Vec::new();
                for i in 0..size {
                    let node = Box::new(TreeNode::leaf(i as i32));
                    nodes.push(node);
                    black_box(&nodes);
                }
                black_box(nodes);
            });
        });
    }
    group.finish();
}

/// Benchmark arena allocation with tree structure (branch nodes)
fn bench_arena_tree_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_tree_allocation");

    for depth in [3, 5, 7] {
        group.throughput(Throughput::Elements(2u64.pow(depth) - 1)); // Complete binary tree nodes
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            b.iter(|| {
                let mut arena = TreeArena::new();
                build_binary_tree(&mut arena, depth);
                black_box(arena);
            });
        });
    }
    group.finish();
}

/// Benchmark Box allocation with tree structure
fn bench_box_tree_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("box_tree_allocation");

    for depth in [3, 5, 7] {
        group.throughput(Throughput::Elements(2u64.pow(depth) - 1));
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            b.iter(|| {
                let tree = build_boxed_tree(depth);
                black_box(tree);
            });
        });
    }
    group.finish();
}

/// Benchmark arena reset and reuse
fn bench_arena_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("arena_reuse");

    for size in [1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut arena = TreeArena::new();
            // Pre-allocate to warm up arena
            for i in 0..size {
                arena.alloc(TreeNode::leaf(i as i32));
            }

            b.iter(|| {
                arena.reset();
                for i in 0..size {
                    let handle = arena.alloc(TreeNode::leaf(i as i32));
                    black_box(handle);
                }
                black_box(&arena);
            });
        });
    }
    group.finish();
}

/// Build a complete binary tree using arena allocation
fn build_binary_tree(arena: &mut TreeArena, depth: u32) {
    fn build_subtree(
        arena: &mut TreeArena,
        depth: u32,
        value: &mut i32,
    ) -> adze::arena_allocator::NodeHandle {
        if depth == 0 {
            let handle = arena.alloc(TreeNode::leaf(*value));
            *value += 1;
            handle
        } else {
            let left = build_subtree(arena, depth - 1, value);
            let right = build_subtree(arena, depth - 1, value);
            arena.alloc(TreeNode::branch(vec![left, right]))
        }
    }

    let mut value = 0;
    build_subtree(arena, depth, &mut value);
}

/// Boxed tree node for comparison
#[derive(Clone)]
#[allow(dead_code)]
enum BoxedTreeNode {
    Leaf { value: i32 },
    Branch { children: Vec<BoxedTreeNode> },
}

/// Build a complete binary tree using Box allocation
fn build_boxed_tree(depth: u32) -> Box<BoxedTreeNode> {
    fn build_subtree(depth: u32, value: &mut i32) -> Box<BoxedTreeNode> {
        if depth == 0 {
            let node = BoxedTreeNode::Leaf { value: *value };
            *value += 1;
            Box::new(node)
        } else {
            let left = build_subtree(depth - 1, value);
            let right = build_subtree(depth - 1, value);
            Box::new(BoxedTreeNode::Branch {
                children: vec![*left, *right],
            })
        }
    }

    let mut value = 0;
    build_subtree(depth, &mut value)
}

criterion_group!(
    benches,
    bench_arena_allocation,
    bench_box_allocation,
    bench_arena_tree_allocation,
    bench_box_tree_allocation,
    bench_arena_reuse
);
criterion_main!(benches);
