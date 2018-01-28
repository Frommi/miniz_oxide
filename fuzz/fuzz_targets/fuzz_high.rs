#![no_main]
#[macro_use]

extern crate libfuzzer_sys;
extern crate miniz_oxide_c_api;

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

    let uncompressed_size = s.len() as c_ulong;

    const N: usize = 1000;

    let mut rust_compressed_size: c_ulong = N as c_ulong;
    let mut rust_compressed_buf = [0u8; N];
    let mut rust_decompressed_size: c_ulong = N as c_ulong;
    let mut rust_decompressed_buf = [0u8; N];

    let mut c_compressed_size: c_ulong = N as c_ulong;
    let mut c_compressed_buf = [0u8; N];
    let mut c_decompressed_size: c_ulong = N as c_ulong;
    let mut c_decompressed_buf = [0u8; N];

    let rust_res =  unsafe {
        miniz_oxide_c_api::mz_compress(rust_compressed_buf.as_mut_ptr(),
                                 &mut rust_compressed_size,
                                 s.as_mut_ptr(),
                                 uncompressed_size)
    };
    let c_res = unsafe {
        c_mz_compress(c_compressed_buf.as_mut_ptr(),
                      &mut c_compressed_size,
                      s.as_mut_ptr(),
                      uncompressed_size)
    };

    assert_eq!(rust_res, c_res);
    assert_eq!(rust_compressed_size, c_compressed_size);
    assert_eq!(rust_compressed_buf[..rust_compressed_size as usize],
               c_compressed_buf[..c_compressed_size as usize]);

    let rust_res = unsafe {
        miniz_oxide_c_api::mz_uncompress(rust_decompressed_buf.as_mut_ptr(),
                                   &mut rust_decompressed_size,
                                   rust_compressed_buf.as_mut_ptr(),
                                   rust_compressed_size)
    };
    let c_res = unsafe {
        c_mz_uncompress(c_decompressed_buf.as_mut_ptr(),
                        &mut c_decompressed_size,
                        c_compressed_buf.as_mut_ptr(),
                        c_compressed_size)
    };

    assert_eq!(rust_res, c_res);
    assert_eq!(rust_decompressed_size, c_decompressed_size);
    assert_eq!(rust_decompressed_buf[..rust_decompressed_size as usize],
               c_decompressed_buf[..c_decompressed_size as usize]);

    assert_eq!(rust_decompressed_size, uncompressed_size);
    assert_eq!(rust_decompressed_buf[..c_decompressed_size as usize],
               s[..c_decompressed_size as usize]);
});
