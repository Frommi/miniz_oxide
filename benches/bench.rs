#![feature(test)]

extern crate miniz_oxide;
extern crate test;

use test::Bencher;
use std::io::Read;

use miniz_oxide::{decompress_to_vec, compress_to_vec};

fn get_test_file_data(name: &str) -> Vec<u8> {
    use std::fs::File;
    let mut input = Vec::new();
    let mut f = File::open(name).unwrap();

    f.read_to_end(&mut input).unwrap();
    input
}

fn get_test_data() -> Vec<u8> {
    use std::env;
    let path = env::var("TEST_FILE").unwrap_or_else(|_| "miniz/miniz.c".to_string());
    get_test_file_data(&path)
}

#[bench]
fn decompress(b: &mut Bencher) {
    let input = get_test_data();

    let compressed = compress_to_vec(input.as_slice(), 6);
    b.iter(||
           decompress_to_vec(&compressed[..])
    );
}

#[bench]
fn compress_fast(b: &mut Bencher) {
    let input = get_test_data();

    b.iter(||
           compress_to_vec(input.as_slice(), 1)
    );
}

#[bench]
fn compress_default(b: &mut Bencher) {
    let input = get_test_data();

    b.iter(||
           compress_to_vec(input.as_slice(), 6)
    );
}

#[bench]
fn compress_high(b: &mut Bencher) {
    let input = get_test_data();

    b.iter(||
           compress_to_vec(input.as_slice(), 9)
    );
}
