use brainfuck_interpreter::interpreter::interpret;
use criterion::{criterion_group, criterion_main, Criterion};
use std::fs;

pub fn criterion_benchmark_calculation(c: &mut Criterion) {
    let url = "./benches/jit_benchmark_test_calculation.bf".to_string();
    let contents = fs::read_to_string(url).expect("Should have been able to read the file");

    c.bench_function("test_with_jit_c", |b| b.iter(|| interpret(&contents, true)));
    c.bench_function("test_without_jit_c", |b| {
        b.iter(|| interpret(&contents, false))
    });
}

pub fn criterion_benchmark_output(c: &mut Criterion) {
    let url = "./benches/jit_benchmark_test_output.bf".to_string();
    let contents = fs::read_to_string(url).expect("Should have been able to read the file");

    c.bench_function("test_with_jit_o", |b| b.iter(|| interpret(&contents, true)));
    c.bench_function("test_without_jit_o", |b| {
        b.iter(|| interpret(&contents, false))
    });
}

criterion_group!(
    benches,
    criterion_benchmark_calculation,
    criterion_benchmark_output
);
criterion_main!(benches);
