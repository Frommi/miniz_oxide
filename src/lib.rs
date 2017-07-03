extern crate libc;

use libc::*;

#[no_mangle]
pub unsafe extern "C" fn mz_adler32(adler: c_ulong, ptr: *const uint8_t, buf_len: size_t) -> c_ulong {
    0
}
