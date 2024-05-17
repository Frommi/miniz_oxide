//! Crate wrapping miniz_oxide in a C API that mimics the C API of the original miniz.
//! Originally designed to allow use of miniz_oxide as a back-end to the
//! [flate2](https://crates.io/crates/flate2) crate.
//!
//! The C API is in a bit of a rough shape currently.
#![allow(clippy::missing_safety_doc)]

extern crate crc32fast;
#[cfg(not(any(
    feature = "libc_stub",
    all(target_arch = "wasm32", not(target_os = "emscripten"))
)))]
extern crate libc;
#[cfg(any(
    feature = "libc_stub",
    all(target_arch = "wasm32", not(target_os = "emscripten"))
))]
mod libc {
    #![allow(non_camel_case_types)]

    use std::alloc::{
        alloc as rust_alloc, dealloc as rust_dealloc, realloc as rust_realloc, Layout,
    };
    use std::mem;

    pub type c_void = u8;
    pub type c_int = i32;
    pub type c_uint = u32;
    pub type c_ulong = u64;
    pub type c_char = i8;
    pub type size_t = usize;

    pub unsafe fn malloc(a: size_t) -> *mut c_void {
        let size = a + mem::size_of::<size_t>();
        let layout = match Layout::from_size_align(size, mem::align_of::<size_t>()) {
            Ok(n) => n,
            Err(_) => return 0 as *mut c_void,
        };
        let ptr = rust_alloc(layout) as *mut size_t;
        *ptr.offset(0) = size;
        ptr.offset(1) as *mut c_void
    }

    pub unsafe fn realloc(ptr: *mut c_void, a: size_t) -> *mut c_void {
        let new_size = a + mem::size_of::<size_t>();
        let ptr = (ptr as *mut size_t).offset(-1);
        let old_size = *ptr.offset(0);
        let layout = Layout::from_size_align_unchecked(old_size, mem::size_of::<size_t>());
        let ptr = rust_realloc(ptr as *mut _, layout, new_size) as *mut size_t;
        *ptr.offset(0) = new_size;
        ptr.offset(1) as *mut c_void
    }

    pub unsafe fn free(ptr: *mut c_void) {
        let ptr = (ptr as *mut size_t).offset(-1);
        let size = *ptr.offset(0);
        let align = mem::size_of::<size_t>();
        let layout = Layout::from_size_align_unchecked(size, align);
        rust_dealloc(ptr as *mut _, layout);
    }
}
extern crate miniz_oxide;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::{cmp, ptr};

use libc::{c_int, c_uint, c_ulong};

use miniz_oxide::deflate::core::CompressionStrategy;
use miniz_oxide::deflate::CompressionLevel;
pub use miniz_oxide::{MZError, MZFlush, MZResult, MZStatus};

pub mod lib_oxide;
use crate::lib_oxide::*;

#[macro_use]
mod unmangle;
mod tdef;
mod tinfl;

mod c_export;
pub use crate::c_export::*;
pub use crate::tdef::Compressor as tdefl_compressor;

pub const MZ_DEFLATED: c_int = 8;
pub use miniz_oxide::MZ_DEFAULT_WINDOW_BITS;

fn as_c_return_code(r: MZResult) -> c_int {
    match r {
        Err(status) => status as c_int,
        Ok(status) => status as c_int,
    }
}

macro_rules! oxidize {
    ($mz_func:ident, $mz_func_oxide:ident; $($arg_name:ident: $type_name:ident),*) => {
        unmangle!(
        pub unsafe extern "C" fn $mz_func(stream: *mut mz_stream, $($arg_name: $type_name),*)
                                          -> c_int {
            match stream.as_mut() {
                None => MZError::Stream as c_int,
                Some(stream) => {
                    // Make sure we catch a potential panic, as
                    // this is called from C.
                    match catch_unwind(AssertUnwindSafe(|| {
                        // Do some checks to see if the stream object has the right type.
                        match StreamOxide::try_new(stream) {
                            Ok(mut stream_oxide) => {
                                let status = $mz_func_oxide(&mut stream_oxide, $($arg_name),*);
                                *stream = stream_oxide.into_mz_stream();
                                as_c_return_code(status) }
                            Err(e) => {
                                e as c_int
                            }
                        }
                    })) {
                        Ok(res) => res,
                        Err(_) => {
                            println!("FATAL ERROR: Caught panic!");
                            MZError::Stream as c_int},
                    }
                }
            }
        });
    };
}

oxidize!(mz_deflate, mz_deflate_oxide;
         flush: c_int);
oxidize!(mz_deflateEnd, mz_deflate_end_oxide;);
oxidize!(mz_deflateReset, mz_deflate_reset_oxide;);

oxidize!(mz_inflate, mz_inflate_oxide;
         flush: c_int);
oxidize!(mz_inflateEnd, mz_inflate_end_oxide;);

unmangle!(
    pub unsafe extern "C" fn mz_deflateInit(stream: *mut mz_stream, level: c_int) -> c_int {
        mz_deflateInit2(
            stream,
            level,
            MZ_DEFLATED,
            MZ_DEFAULT_WINDOW_BITS,
            9,
            CompressionStrategy::Default as c_int,
        )
    }

    pub unsafe extern "C" fn mz_deflateInit2(
        stream: *mut mz_stream,
        level: c_int,
        method: c_int,
        window_bits: c_int,
        mem_level: c_int,
        strategy: c_int,
    ) -> c_int {
        match stream.as_mut() {
            None => MZError::Stream as c_int,
            Some(stream) => {
                stream.data_type = StateTypeEnum::DeflateType;
                // Make sure we catch a potential panic, as
                // this is called from C.
                match catch_unwind(AssertUnwindSafe(|| match StreamOxide::try_new(stream) {
                    Ok(mut stream_oxide) => {
                        let status = mz_deflate_init2_oxide(
                            &mut stream_oxide,
                            level,
                            method,
                            window_bits,
                            mem_level,
                            strategy,
                        );
                        *stream = stream_oxide.into_mz_stream();
                        as_c_return_code(status)
                    }
                    Err(e) => e as c_int,
                })) {
                    Ok(res) => res,
                    Err(_) => {
                        println!("FATAL ERROR: Caught panic!");
                        MZError::Stream as c_int
                    }
                }
            }
        }
    }

    pub unsafe extern "C" fn mz_inflateInit2(stream: *mut mz_stream, window_bits: c_int) -> c_int {
        match stream.as_mut() {
            None => MZError::Stream as c_int,
            Some(stream) => {
                stream.data_type = StateTypeEnum::InflateType;
                // Make sure we catch a potential panic, as
                // this is called from C.
                match catch_unwind(AssertUnwindSafe(|| match StreamOxide::try_new(stream) {
                    Ok(mut stream_oxide) => {
                        let status = mz_inflate_init2_oxide(&mut stream_oxide, window_bits);
                        *stream = stream_oxide.into_mz_stream();
                        as_c_return_code(status)
                    }
                    Err(e) => e as c_int,
                })) {
                    Ok(res) => res,
                    Err(_) => {
                        println!("FATAL ERROR: Caught panic!");
                        MZError::Stream as c_int
                    }
                }
            }
        }
    }

    pub unsafe extern "C" fn mz_compress(
        dest: *mut u8,
        dest_len: *mut c_ulong,
        source: *const u8,
        source_len: c_ulong,
    ) -> c_int {
        mz_compress2(
            dest,
            dest_len,
            source,
            source_len,
            CompressionLevel::DefaultCompression as c_int,
        )
    }

    pub unsafe extern "C" fn mz_compress2(
        dest: *mut u8,
        dest_len: *mut c_ulong,
        source: *const u8,
        source_len: c_ulong,
        level: c_int,
    ) -> c_int {
        dest_len
            .as_mut()
            .map_or(MZError::Param as c_int, |dest_len| {
                if buffer_too_large(source_len, *dest_len) {
                    return MZError::Param as c_int;
                }

                let mut stream: mz_stream = mz_stream {
                    next_in: source,
                    avail_in: source_len as c_uint,
                    next_out: dest,
                    avail_out: (*dest_len) as c_uint,
                    data_type: StateTypeEnum::DeflateType,
                    ..Default::default()
                };

                let mut stream_oxide = StreamOxide::new(&mut stream);
                as_c_return_code(mz_compress2_oxide(&mut stream_oxide, level, dest_len))
            })
    }

    pub extern "C" fn mz_deflateBound(_stream: *mut mz_stream, source_len: c_ulong) -> c_ulong {
        cmp::max(
            128 + (source_len * 110) / 100,
            128 + source_len + ((source_len / (31 * 1024)) + 1) * 5,
        )
    }

    pub unsafe extern "C" fn mz_inflateInit(stream: *mut mz_stream) -> c_int {
        mz_inflateInit2(stream, MZ_DEFAULT_WINDOW_BITS)
    }

    pub unsafe extern "C" fn mz_uncompress(
        dest: *mut u8,
        dest_len: *mut c_ulong,
        source: *const u8,
        source_len: c_ulong,
    ) -> c_int {
        dest_len
            .as_mut()
            .map_or(MZError::Param as c_int, |dest_len| {
                if buffer_too_large(source_len, *dest_len) {
                    return MZError::Param as c_int;
                }

                let mut stream: mz_stream = mz_stream {
                    next_in: source,
                    avail_in: source_len as c_uint,
                    next_out: dest,
                    avail_out: (*dest_len) as c_uint,
                    data_type: StateTypeEnum::InflateType,
                    ..Default::default()
                };

                // We don't expect this to fail since we supply the stream ourselves.
                let mut stream_oxide = StreamOxide::new(&mut stream);
                as_c_return_code(mz_uncompress2_oxide(&mut stream_oxide, dest_len))
            })
    }

    pub extern "C" fn mz_compressBound(source_len: c_ulong) -> c_ulong {
        mz_deflateBound(ptr::null_mut(), source_len)
    }
);

#[cfg(target_bit_width = "64")]
#[inline]
fn buffer_too_large(source_len: c_ulong, dest_len: c_ulong) -> bool {
    (source_len | dest_len) > 0xFFFFFFFF
}

#[cfg(not(target_bit_width = "64"))]
#[inline]
fn buffer_too_large(_source_len: c_ulong, _dest_len: c_ulong) -> bool {
    false
}
