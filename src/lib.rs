extern crate libc;

use std::slice;
use self::libc::*;
use std::ptr;
use std::mem;
use std::cmp;

mod lib_oxide;
pub use lib_oxide::*;

mod tdef;
pub use tdef::tdefl_radix_sort_syms;
pub use tdef::tdefl_create_comp_flags_from_zip_params;
pub use tdef::tdefl_compressor;
pub use tdef::tdefl_put_buf_func_ptr;

mod tinfl;
pub use tinfl::tinfl_decompressor;

pub const MZ_ADLER32_INIT: c_ulong = 1;
pub const MZ_CRC32_INIT: c_ulong = 0;

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

pub const MZ_NO_COMPRESSION: c_int = 0;
pub const MZ_BEST_SPEED: c_int = 1;
pub const MZ_BEST_COMPRESSION: c_int = 9;
pub const MZ_UBER_COMPRESSION: c_int = 10;
pub const MZ_DEFAULT_LEVEL: c_int = 6;
pub const MZ_DEFAULT_COMPRESSION: c_int = -1;

pub const MZ_DEFAULT_STRATEGY: c_int = 0;
pub const MZ_FILTERED: c_int = 1;
pub const MZ_HUFFMAN_ONLY: c_int = 2;
pub const MZ_RLE: c_int = 3;
pub const MZ_FIXED: c_int = 4;


#[allow(bad_style)]
extern {
    pub fn miniz_def_alloc_func(opaque: *mut c_void, items: size_t, size: size_t) -> *mut c_void;
    pub fn miniz_def_free_func(opaque: *mut c_void, address: *mut c_void);
}


#[no_mangle]
pub unsafe extern "C" fn mz_adler32(adler: c_ulong, ptr: *const u8, buf_len: usize) -> c_ulong {
    match ptr.as_ref() {
        None => MZ_ADLER32_INIT,
        Some(r) => {
            let data = slice::from_raw_parts(r, buf_len);
            mz_adler32_oxide(adler, data)
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn mz_crc32(crc: c_ulong, ptr: *const u8, buf_len: size_t) -> c_ulong {
    match ptr.as_ref() {
        None => MZ_CRC32_INIT,
        Some(r) => {
            let data = slice::from_raw_parts(r, buf_len);
            mz_crc32_oxide(crc as c_uint, data) as c_ulong
        }
    }
}

#[allow(bad_style)]
pub enum mz_internal_state {}
#[allow(bad_style)]
pub type mz_alloc_func = unsafe extern "C" fn(*mut c_void, size_t, size_t) -> *mut c_void;
#[allow(bad_style)]
pub type mz_free_func = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
#[derive(Debug)]
#[allow(bad_style)]
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

macro_rules! oxidize {
    ($mz_func:ident, $mz_func_oxide:ident, $($arg_name:ident: $type_name:ident),*) => {
        #[no_mangle]
        #[allow(bad_style)]
        pub unsafe extern "C" fn $mz_func(stream: *mut mz_stream, $($arg_name : $type_name),*) -> c_int {
            match stream.as_mut() {
                None => MZ_STREAM_ERROR,
                Some(stream) => {
                    let mut stream_oxide = StreamOxide::new(&mut *stream);
                    let status = lib_oxide::$mz_func_oxide(&mut stream_oxide, $($arg_name),*);
                    *stream = stream_oxide.as_mz_stream();
                    status
                }
            }
        }
    };
}

oxidize!(mz_deflateInit2, mz_deflate_init2_oxide,
         level: c_int, method: c_int, window_bits: c_int, mem_level: c_int, strategy: c_int);
oxidize!(mz_deflate, mz_deflate_oxide,
         flush: c_int);
oxidize!(mz_deflateEnd, mz_deflate_end_oxide, );
oxidize!(mz_deflateReset, mz_deflate_reset_oxide, );


oxidize!(mz_inflateInit2, mz_inflate_init2_oxide,
         window_bits: c_int);
oxidize!(mz_inflate, mz_inflate_oxide,
         flush: c_int);
oxidize!(mz_inflateEnd, mz_inflate_end_oxide, );


#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn mz_deflateInit(stream: *mut mz_stream, level: c_int) -> c_int {
    mz_deflateInit2(stream, level, MZ_DEFLATED, MZ_DEFAULT_WINDOW_BITS, 9, MZ_DEFAULT_STRATEGY)
}

#[no_mangle]
pub unsafe extern "C" fn mz_compress(dest: *mut u8,
                                     dest_len: *mut c_ulong,
                                     source: *const u8,
                                     source_len: c_ulong) -> c_int
{
    mz_compress2(dest, dest_len, source, source_len, MZ_DEFAULT_COMPRESSION)
}

#[no_mangle]
pub unsafe extern "C" fn mz_compress2(dest: *mut u8,
                                      dest_len: *mut c_ulong,
                                      source: *const u8,
                                      source_len: c_ulong,
                                      level: c_int) -> c_int
{
    match dest_len.as_mut() {
        None => return MZ_PARAM_ERROR,
        Some(dest_len) => {
            if (source_len | *dest_len) > 0xFFFFFFFF {
                return MZ_PARAM_ERROR;
            }

            let mut stream: mz_stream = mz_stream {
                next_in: source,
                avail_in: source_len as c_uint,
                next_out: dest,
                avail_out: (*dest_len) as c_uint,
                ..Default::default()
            };

            let mut stream_oxide = StreamOxide::new(&mut stream);
            mz_compress2_oxide(&mut stream_oxide, level, dest_len)
        }
    }
}

#[no_mangle]
#[allow(bad_style, unused_variables)]
pub extern "C" fn mz_deflateBound(stream: *mut mz_stream, source_len: c_ulong) -> c_ulong {
    cmp::max(128 + (source_len * 110) / 100, 128 + source_len + ((source_len / (31 * 1024)) + 1) * 5)
}


#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn mz_inflateInit(stream: *mut mz_stream) -> c_int {
    mz_inflateInit2(stream, MZ_DEFAULT_WINDOW_BITS)
}

#[no_mangle]
pub unsafe extern "C" fn mz_uncompress(dest: *mut u8,
                                       dest_len: *mut c_ulong,
                                       source: *const u8,
                                       source_len: c_ulong) -> c_int
{
    match dest_len.as_mut() {
        None => return MZ_PARAM_ERROR,
        Some(dest_len) => {
            if (source_len | *dest_len) > 0xFFFFFFFF {
                return MZ_PARAM_ERROR;
            }

            let mut stream: mz_stream = mz_stream {
                next_in: source,
                avail_in: source_len as c_uint,
                next_out: dest,
                avail_out: (*dest_len) as c_uint,
                ..Default::default()
            };

            let mut stream_oxide = StreamOxide::new(&mut stream);
            mz_uncompress2_oxide(&mut stream_oxide, dest_len)
        }
    }
}

#[no_mangle]
#[allow(bad_style)]
pub extern "C" fn mz_compressBound(source_len: c_ulong) -> c_ulong {
    mz_deflateBound(ptr::null_mut(), source_len)
}
