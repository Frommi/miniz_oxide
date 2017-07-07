#![cfg_attr(feature="afl_test", feature(plugin))]
#![cfg_attr(feature="afl_test", plugin(afl_plugin))]

#[cfg(feature="afl_test")]
extern crate afl;

extern crate libc;
extern crate miniz_oxide;

use self::libc::*;
use std::io;
use std::io::BufRead;

#[cfg(feature="afl_test")]
fn main() {
    afl::handle_string(|s| {
        if s.len() > 1024 {
            println!("long test");
        } else {
            let s_cp = s.clone();

            let mut cmp_size: c_ulong = 2048;
            let mut decmp_size: c_ulong = 2048;
            let mut cmp_buf = [0u8; 2048];
            let mut decmp_buf = [0u8; 2048];
            let mut uncmp_size = s.len() as c_ulong;
            let mut status = unsafe { miniz_oxide::mz_compress(cmp_buf.as_mut_ptr(), &mut cmp_size, s.into_bytes().as_mut_ptr(), uncmp_size) };
            assert!(status == miniz_oxide::MZ_OK);
            status = unsafe { miniz_oxide::mz_uncompress(decmp_buf.as_mut_ptr(), &mut decmp_size, cmp_buf.as_mut_ptr(), cmp_size) };
            assert!(status == miniz_oxide::MZ_OK);
            assert!(decmp_size == uncmp_size);
            let t = s_cp.into_bytes();
            for i in 0..decmp_size {
                assert!(decmp_buf[i as usize] == t[i as usize]);
            }
        }
    })
}

#[cfg(not(feature="afl_test"))]
fn main() {}
