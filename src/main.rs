#![cfg_attr(feature="afl_test", feature(plugin))]
#![cfg_attr(feature="afl_test", plugin(afl_plugin))]

#[cfg(feature="afl_test")]
extern crate afl;

mod lib;

use std::io;
use std::io::BufRead;

#[cfg(feature="afl_test")]
fn main() {
    afl::handle_string(|s| {
        lib::mz_adler32_oxide(239, &(s.into_bytes()));
    })
}

#[cfg(not(feature="afl_test"))]
fn main() {}
