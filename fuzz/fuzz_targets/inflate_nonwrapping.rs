#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate flate2;

use flate2::read::DeflateDecoder;
use std::io::Read;

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here
    let mut result = Vec::new();
    let _ = DeflateDecoder::new(data).read_to_end(&mut result);
});
