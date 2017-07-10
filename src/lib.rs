extern crate libc;

use std::slice;
use self::libc::*;
use std::ptr;

mod tdef;
pub use tdef::tdefl_radix_sort_syms;


pub const MZ_ADLER32_INIT: c_ulong = 1;

pub const MZ_NO_FLUSH: c_int = 0;
pub const MZ_PARTIAL_FLUSH: c_int = 1;
pub const MZ_SYNC_FLUSH: c_int = 2;
pub const MZ_FULL_FLUSH: c_int = 3;
pub const MZ_FINISH: c_int = 4;
pub const MZ_BLOCK: c_int = 5;

pub const MZ_OK: c_int = 0;
pub const MZ_STREAM_END: c_int = 1;
pub const MZ_NEED_DICT: c_int = 2;
pub const MZ_ERRNO: c_int = -1;
pub const MZ_STREAM_ERROR: c_int = -2;
pub const MZ_DATA_ERROR: c_int = -3;
pub const MZ_MEM_ERROR: c_int = -4;
pub const MZ_BUF_ERROR: c_int = -5;
pub const MZ_VERSION_ERROR: c_int = -6;
pub const MZ_PARAM_ERROR: c_int = -10000;

pub const MZ_DEFLATED: c_int = 8;
pub const MZ_DEFAULT_WINDOW_BITS: c_int = 15;
pub const MZ_DEFAULT_STRATEGY: c_int = 0;

pub const MZ_DEFAULT_COMPRESSION: c_int = 6;


#[no_mangle]
pub unsafe extern "C" fn mz_adler32(adler: c_ulong, ptr: *const u8, buf_len: usize) -> c_ulong {
    if ptr.is_null() {
        MZ_ADLER32_INIT
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


pub enum mz_internal_state {}
pub type mz_alloc_func = extern "C" fn(*mut c_void, size_t, size_t) -> *mut c_void;
pub type mz_free_func = extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
pub struct mz_stream {
    pub next_in: *const u8,
    pub avail_in: c_uint,
    pub total_in: c_ulong,

    pub next_out: *mut u8,
    pub avail_out: c_uint,
    pub total_out: c_ulong,

    pub msg: *const c_char,
    pub state: *mut mz_internal_state,

    pub zalloc: Option<mz_alloc_func>,
    pub zfree: Option<mz_free_func>,
    pub opaque: *mut c_void,

    pub data_type: c_int,
    pub adler: c_ulong,
    pub reserved: c_ulong,
}

impl Default for mz_stream {
    fn default () -> mz_stream {
        mz_stream {
            next_in: ptr::null(),
            avail_in: 0,
            total_in: 0,

            next_out: ptr::null_mut(),
            avail_out: 0,
            total_out: 0,

            msg: ptr::null(),
            state: ptr::null_mut(),

            zalloc: None,
            zfree: None,
            opaque: ptr::null_mut(),

            data_type: 0,
            adler: 0,
            reserved: 0,
        }
    }
}

pub struct StreamOxide<'s> {
    stream: &'s mut mz_stream,
}

impl<'s> StreamOxide<'s> {
    pub fn reduce(&mut self) -> &mut mz_stream {
        self.stream
    }
}


extern {
    pub fn mz_deflateInit(stream: *mut mz_stream, level: c_int) -> c_int;
    pub fn mz_deflate(stream: *mut mz_stream, flush: c_int) -> c_int;
    pub fn mz_deflateEnd(stream: *mut mz_stream) -> c_int;
    pub fn mz_compressBound(source_len: c_ulong) -> c_ulong;
    pub fn mz_uncompress(pDest: *mut u8, pDest_len: *mut c_ulong, pSource: *const u8, source_len: c_ulong) -> c_int;
}

#[no_mangle]
pub unsafe extern "C" fn mz_compress(pDest: *mut u8, pDest_len: *mut c_ulong, pSource: *const u8, source_len: c_ulong) -> c_int {
    mz_compress2(pDest, pDest_len, pSource, source_len, MZ_DEFAULT_COMPRESSION)
}

#[no_mangle]
pub unsafe extern "C" fn mz_compress2(pDest: *mut u8, pDest_len: *mut c_ulong, pSource: *const u8, source_len: c_ulong, level: c_int) -> c_int {
    assert!(!pDest_len.is_null());
    if (source_len | *pDest_len) > 0xFFFFFFFF {
        return MZ_PARAM_ERROR;
    }

    let mut stream : mz_stream = mz_stream {
        next_in: pSource,
        avail_in: source_len as c_uint,
        next_out: pDest,
        avail_out: (*pDest_len) as c_uint,
        ..Default::default()
    };

    let mut stream_oxide = StreamOxide { stream : &mut stream };
    let status = mz_compress2_oxide(&mut stream_oxide, level);
    *pDest_len = stream_oxide.reduce().total_out;

    status
}

pub fn mz_compress2_oxide(stream_oxide: &mut StreamOxide, level: c_int) -> c_int {
    let mut status: c_int = unsafe { mz_deflateInit(stream_oxide.reduce(), level) };
    if status != MZ_OK {
        return status;
    }

    status = unsafe { mz_deflate(stream_oxide.reduce(), MZ_FINISH) };
    if status != MZ_STREAM_END {
        unsafe { mz_deflateEnd(stream_oxide.reduce()) };
        return if status == MZ_OK { MZ_BUF_ERROR } else { status };
    }

    unsafe { mz_deflateEnd(stream_oxide.reduce()) }
}
