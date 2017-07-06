extern crate libc;

use std::slice;
use self::libc::*;

#[no_mangle]
pub unsafe extern "C" fn mz_adler32(adler: c_ulong, ptr: *const u8, buf_len: usize) -> c_ulong {
    if ptr.is_null() {
        1
    } else {
        let data_slice = slice::from_raw_parts(ptr, buf_len);
        mz_adler32_oxide(adler, data_slice)
    }
}

pub fn mz_adler32_oxide(adler: c_ulong, data: &[u8]) -> c_ulong {
    let mut s1 = adler & 0xffff;
    let mut s2 = adler >> 16;
    for x in data {
        s1 = (s1 + *x as c_ulong) % 65521;
        s2 = (s1 + s2) % 65521;
    }
    (s2 << 16) + s1
}
