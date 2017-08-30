#![feature(test)]

extern crate libc;
extern crate miniz_oxide;
extern crate test;

use test::Bencher;
use std::io::Read;
use libc::{c_void, c_int};

use miniz_oxide::{
    decompress_to_vec,
    compress_to_vec,
    compress_to_vec_zlib,
    decompress_to_vec_zlib,

    tdefl_create_comp_flags_from_zip_params,
    tdefl_compress_mem_to_heap,
};

extern "C" {
    fn c_tinfl_decompress_mem_to_heap(
        src_buf: *const c_void,
        src_buf_len: usize,
        out_len: *mut usize,
        flags: c_int,
    ) -> *mut c_void;

    fn c_tdefl_compress_mem_to_heap(
        src_buf: *const c_void,
        src_buf_len: usize,
        out_len: *mut usize,
        flags: c_int,
    ) -> *mut c_void;
}

fn get_test_file_data(name: &str) -> Vec<u8> {
    use std::fs::File;
    let mut input = Vec::new();
    let mut f = File::open(name).unwrap();

    f.read_to_end(&mut input).unwrap();
    input
}

fn get_test_data() -> Vec<u8> {
    use std::env;
    let path = env::var("TEST_FILE").unwrap_or_else(|_| "bin/libminiz.a".to_string());
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
fn decompress_mem_to_heap_miniz(b: &mut Bencher) {
    let input = get_test_data();
    let compressed = compress_to_vec(input.as_slice(), 6);

    let mut out_len: usize = 0;
    b.iter(||
        unsafe {
            c_tinfl_decompress_mem_to_heap(
                compressed.as_ptr() as *mut c_void,
                compressed.len(),
                &mut out_len,
                0,
            )
        }
    );
}

#[bench]
fn zlib_decompress(b: &mut Bencher) {
    let input = get_test_data();

    let compressed = compress_to_vec_zlib(input.as_slice(), 6);
    b.iter(||
        decompress_to_vec_zlib(&compressed[..])
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
fn compress_mem_to_heap_fast_oxide(b: &mut Bencher) {
    let input = get_test_data();

    let mut out_len: usize = 0;
    let flags = tdefl_create_comp_flags_from_zip_params(0, -15, 0) as i32;
    b.iter(||
        unsafe {
            tdefl_compress_mem_to_heap(
                input.as_ptr() as *mut c_void,
                input.len(),
                &mut out_len,
                flags,
            )
        }
    );
}

#[bench]
fn compress_mem_to_heap_default_oxide(b: &mut Bencher) {
    let input = get_test_data();

    let mut out_len: usize = 0;
    let flags = tdefl_create_comp_flags_from_zip_params(6, -15, 0) as i32;
    b.iter(||
        unsafe {
            tdefl_compress_mem_to_heap(
                input.as_ptr() as *mut c_void,
                input.len(),
                &mut out_len,
                flags,
            )
        }
    );
}

#[bench]
fn compress_mem_to_heap_high_oxide(b: &mut Bencher) {
    let input = get_test_data();

    let mut out_len: usize = 0;
    let flags = tdefl_create_comp_flags_from_zip_params(9, -15, 0) as i32;
    b.iter(||
        unsafe {
            tdefl_compress_mem_to_heap(
                input.as_ptr() as *mut c_void,
                input.len(),
                &mut out_len,
                flags,
            )
        }
    );
}

#[bench]
fn compress_mem_to_heap_fast_miniz(b: &mut Bencher) {
    let input = get_test_data();

    let mut out_len: usize = 0;
    let flags = tdefl_create_comp_flags_from_zip_params(0, -15, 0) as i32;
    b.iter(||
        unsafe {
           c_tdefl_compress_mem_to_heap(
               input.as_ptr() as *mut c_void,
               input.len(),
               &mut out_len,
               flags,
           )
        }
    );
}

#[bench]
fn compress_mem_to_heap_default_miniz(b: &mut Bencher) {
    let input = get_test_data();

    let mut out_len: usize = 0;
    let flags = tdefl_create_comp_flags_from_zip_params(6, -15, 0) as i32;
    b.iter(||
        unsafe {
            c_tdefl_compress_mem_to_heap(
                input.as_ptr() as *mut c_void,
                input.len(),
                &mut out_len,
                flags,
            )
        }
    );
}

#[bench]
fn compress_mem_to_heap_high_miniz(b: &mut Bencher) {
    let input = get_test_data();

    let mut out_len: usize = 0;
    let flags = tdefl_create_comp_flags_from_zip_params(9, -15, 0) as i32;
    b.iter(||
        unsafe {
            c_tdefl_compress_mem_to_heap(
                input.as_ptr() as *mut c_void,
                input.len(),
                &mut out_len,
                flags,
            )
        }
    );
}

#[bench]
fn zlib_compress_fast(b: &mut Bencher) {
    let input = get_test_data();

    b.iter(||
        compress_to_vec_zlib(input.as_slice(), 1)
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
