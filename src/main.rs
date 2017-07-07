#![cfg_attr(feature="afl_test", feature(plugin))]
#![cfg_attr(feature="afl_test", plugin(afl_plugin))]

#[cfg(feature="afl_test")]
extern crate afl;

extern crate miniz_oxide;


use std::io;
use std::io::BufRead;

#[cfg(feature="afl_test")]
fn main() {
    afl::handle_string(|s| {
        miniz_oxide::mz_adler32_oxide(239, &(s.into_bytes()));
    })
}

#[cfg(not(feature="afl_test"))]
fn main() {}
