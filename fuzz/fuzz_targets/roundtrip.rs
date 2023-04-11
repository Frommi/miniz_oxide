#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate miniz_oxide;

fuzz_target!(|input: (u8, Vec<u8>)| {
    let compression_level = input.0;
    let data = input.1;
    let compressed = miniz_oxide::deflate::compress_to_vec(&data, compression_level);
    let decompressed = miniz_oxide::inflate::decompress_to_vec(&compressed).expect("Failed to decompress compressed data!");
    assert_eq!(data, decompressed);
});
