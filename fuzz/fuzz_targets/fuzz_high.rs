#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate miniz_oxide;

extern crate libc;

use libc::*;

extern "C" {
    pub fn c_mz_compress(dest: *mut u8,
                         dest_len: *mut c_ulong,
                         source: *const u8,
                         source_len: c_ulong) -> c_int;
    pub fn c_mz_uncompress(dest: *mut u8,
                           dest_len: *mut c_ulong,
                           source: *const u8,
                           source_len: c_ulong) -> c_int;
}

fuzz_target!(|data: &[u8]| {
    let mut s = data.to_vec();
    if s.len() > 1024 {
        println!("long test");
    } else {
        let s_cp = s.clone();

        let uncmp_size = s.len() as c_ulong;
        let mut cmp_size: c_ulong = 2048;
        let mut decmp_size: c_ulong = 2048;
        let mut cmp_buf = [0u8; 2048];
        let mut decmp_buf = [0u8; 2048];

        let c_res = unsafe {
            c_mz_compress(cmp_buf.as_mut_ptr(), &mut cmp_size, s.as_mut_ptr(), uncmp_size)
        };
        let rust_res =  unsafe {
            miniz_oxide::mz_compress(cmp_buf.as_mut_ptr(), &mut cmp_size, s.as_mut_ptr(), uncmp_size)
        };

        assert!(c_res == rust_res);

        let c_res = unsafe {
            c_mz_uncompress(decmp_buf.as_mut_ptr(), &mut decmp_size, cmp_buf.as_mut_ptr(), cmp_size)
        };
        let rust_res = unsafe {
            miniz_oxide::mz_uncompress(decmp_buf.as_mut_ptr(), &mut decmp_size, cmp_buf.as_mut_ptr(), cmp_size)
        };

        assert!(c_res == rust_res);

        assert!(decmp_size == uncmp_size);
        for i in 0..decmp_size {
            assert!(decmp_buf[i as usize] == s_cp[i as usize]);
        }
    }
});
