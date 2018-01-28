#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate miniz_oxide;

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here
    let _ = miniz_oxide::inflate::decompress_to_vec(data);
});
