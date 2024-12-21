extern crate criterion;

use std::hint::black_box;
use std::io::Read;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};
use miniz_oxide::inflate::{decompress_to_vec, decompress_to_vec_zlib, TINFLStatus};

fn get_test_file_data(name: &str) -> Vec<u8> {
    use std::fs::File;
    let mut input = Vec::new();
    let mut f = File::open(name).unwrap();

    f.read_to_end(&mut input).unwrap();
    input
}

fn get_test_data() -> Vec<u8> {
    use std::env;
    let path = env::var("TEST_FILE").unwrap_or_else(|_| "../miniz/miniz.c".to_string());
    get_test_file_data(&path)
}

fn bench_inflate(c: &mut Criterion) {
    let data = get_test_data();
    let compressed= compress_to_vec(&data, 6);
    c.bench_function("inflate", |b| b.iter(|| decompress_to_vec(black_box(&compressed))));
    let compressed_zlib = compress_to_vec_zlib(&data, 6);
    c.bench_function("inflate_zlib", |b| b.iter(|| decompress_to_vec_zlib(black_box(&compressed_zlib))));
}

fn bench_deflate(c: &mut Criterion) {
    let data = get_test_data();
    c.bench_function("deflate_l6", |b| b.iter(|| compress_to_vec(black_box(&data), 6)));
    c.bench_function("deflate_zlib_l6", |b| b.iter(|| compress_to_vec_zlib(black_box(&data), 6)));
}

criterion_group!(benches, bench_inflate, bench_deflate);
criterion_main!(benches);