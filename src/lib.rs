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
pub use tdef::tdefl_compressor;
pub use tdef::tdefl_put_buf_func_ptr;

mod tinfl;
pub use tinfl::tinfl_decompressor;

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

#[allow(bad_style)]
pub enum mz_internal_state {}
#[allow(bad_style)]
pub type mz_alloc_func = unsafe extern "C" fn(*mut c_void, size_t, size_t) -> *mut c_void;
#[allow(bad_style)]
pub type mz_free_func = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
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

pub fn write_mz_stream(stream: &mz_stream) {
    println!("next_in: {}", stream.next_in as usize);
    println!("avail_in: {}", stream.avail_in);
    println!("total_in: {}", stream.total_in);
    println!();
    println!("next_out: {}", stream.next_out as usize);
    println!("avail_out: {}", stream.avail_out);
    println!("total_out: {}", stream.total_out);
    println!();
    println!("msg");
    println!("state: {}", stream.state as usize);
    println!();
    println!("zalloc");
    println!("zfree");
    println!("opaque");
    println!();
    println!("data_type: {}", stream.data_type);
    println!("adler: {}", stream.adler);
    println!("reserved: {}", stream.reserved);
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


#[allow(bad_style)]
extern {
    pub fn miniz_def_alloc_func(opaque: *mut c_void, items: size_t, size: size_t) -> *mut c_void;
    pub fn miniz_def_free_func(opaque: *mut c_void, address: *mut c_void);

    pub fn mz_inflate(stream: *mut mz_stream, flush: c_int) -> c_int;
    pub fn mz_inflateEnd(stream: *mut mz_stream) -> c_int;

    pub fn tdefl_create_comp_flags_from_zip_params(level: c_int, window_bits: c_int, strategy: c_int) -> c_uint;
    pub fn tdefl_init(d: *mut tdefl_compressor, pPut_buf_func: Option<tdefl_put_buf_func_ptr>,
                      pPut_buf_user: *mut c_void, flags: c_int) -> c_int;
    pub fn tdefl_compress(d: *mut tdefl_compressor, pIn_buf: *const c_void, pIn_buf_size: *mut size_t,
                          pOut_buf: *mut c_void, pOut_buf_size: *mut size_t, flush: c_int) -> c_int;
}

#[no_mangle]
pub unsafe extern "C" fn mz_compress(dest: *mut u8, dest_len: *mut c_ulong,
                                     source: *const u8, source_len: c_ulong) -> c_int {
    mz_compress2(dest, dest_len, source, source_len, MZ_DEFAULT_COMPRESSION)
}

#[no_mangle]
pub unsafe extern "C" fn mz_compress2(dest: *mut u8, dest_len: *mut c_ulong,
                                      source: *const u8, source_len: c_ulong, level: c_int) -> c_int {
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
#[allow(bad_style)]
pub unsafe extern "C" fn mz_deflateInit(stream: *mut mz_stream, level: c_int) -> c_int {
    mz_deflateInit2(stream, level, MZ_DEFLATED, MZ_DEFAULT_WINDOW_BITS, 9, MZ_DEFAULT_STRATEGY)
}

#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn mz_deflateInit2(stream: *mut mz_stream, level: c_int, method: c_int,
                                         window_bits: c_int, mem_level: c_int, strategy: c_int) -> c_int {
    match stream.as_mut() {
        None => MZ_STREAM_ERROR,
        Some(stream) => {
            let mut stream_oxide = StreamOxide::new(&mut *stream);
            let status = lib_oxide::mz_deflate_init2_oxide(
                &mut stream_oxide, level, method, window_bits, mem_level, strategy);
            *stream = stream_oxide.as_mz_stream();
            status
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn mz_deflate(stream: *mut mz_stream, flush: c_int) -> c_int {
    match stream.as_mut() {
        None => MZ_STREAM_ERROR,
        Some(stream) => {
            let mut stream_oxide = StreamOxide::new(&mut *stream);
            let status = mz_deflate_oxide(&mut stream_oxide, flush);
            *stream = stream_oxide.as_mz_stream();
            status
        }
    }
}

#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn mz_deflateEnd(stream: *mut mz_stream) -> c_int {
    match stream.as_mut() {
        None => MZ_STREAM_ERROR,
        Some(stream) => {
            let mut stream_oxide = StreamOxide::new(&mut *stream);
            let status = mz_deflate_end_oxide(&mut stream_oxide);
            *stream = stream_oxide.as_mz_stream();
            status
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
pub extern "C" fn mz_compressBound(source_len: c_ulong) -> c_ulong {
    mz_deflateBound(ptr::null_mut(), source_len)
}

use tinfl::TINFL_LZ_DICT_SIZE;
use tinfl::TINFL_STATUS_NEEDS_MORE_INPUT;

#[repr(C)]
#[allow(bad_style)]
pub struct inflate_state {
    pub m_decomp: tinfl_decompressor,

    pub m_dict_ofs: c_uint,
    pub m_dict_avail: c_uint,
    pub m_first_call: c_uint,
    pub m_has_flushed: c_uint,

    pub m_window_bits: c_int,
    pub m_dict: [u8; TINFL_LZ_DICT_SIZE],
    pub m_last_status: c_int
}

#[no_mangle]
pub unsafe extern "C" fn mz_uncompress(dest: *mut u8, dest_len: *mut c_ulong,
                                       source: *const u8, source_len: c_ulong) -> c_int {
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
pub unsafe extern "C" fn mz_inflateInit(stream: *mut mz_stream) -> c_int {
    mz_inflateInit2(stream, MZ_DEFAULT_WINDOW_BITS)
}

#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn mz_inflateInit2(stream: *mut mz_stream, window_bits: c_int) -> c_int {
    match stream.as_mut() {
        None => MZ_STREAM_ERROR,
        Some(stream) => {
            let mut stream_oxide = StreamOxide::new(&mut *stream);
            let status = lib_oxide::mz_inflate_init2_oxide(&mut stream_oxide, window_bits);
            *stream = stream_oxide.as_mz_stream();
            status
        }
    }
}
