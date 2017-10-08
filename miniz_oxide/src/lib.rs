//! A pure rust replacement for the [miniz](https://github.com/richgel999/miniz)
//! DEFLATE/zlib encoder/decoder.
//! The plan for this crate is to be used as a back-end for the
//! [flate2](https://github.com/alexcrichton/flate2-rs) crate and eventually remove the
//! need to depend on a C library.
//!
//! # Usage
//! ## Simple compression/decompression:
//! ``` rust
//!
//! use miniz_oxide::inflate::decompress_to_vec;
//! use miniz_oxide::deflate::compress_to_vec;
//!
//! fn roundtrip(data: &[u8]) {
//!     let compressed = compress_to_vec(data, 6);
//!     let decompressed = decompress_to_vec(compressed.as_slice()).expect("Failed to decompress!");
//! #   let _ = decompressed;
//! }
//!
//! # roundtrip(b"Test_data test data lalalal blabla");
//!
//! ```

extern crate adler32;
extern crate libc;

pub mod inflate;
pub mod deflate;
mod shared;

pub use shared::update_adler32 as mz_adler32_oxide;
pub use shared::MZ_ADLER32_INIT;

use libc::{c_int, c_void, size_t};


/// Unused opaque pointer.
#[allow(bad_style)]
pub enum mz_internal_state {}
/// Signature of function used to allocate the compressor/decompressor structs.
#[allow(bad_style)]
pub type mz_alloc_func = unsafe extern "C" fn(*mut c_void, size_t, size_t) -> *mut c_void;
/// Signature of function used to free the compressor/decompressor structs.
#[allow(bad_style)]
pub type mz_free_func = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MZFlush {
    None = 0,
    Partial = 1,
    Sync = 2,
    Full = 3,
    Finish = 4,
    Block = 5,
}

impl MZFlush {
    /// Create a Flush instance from an integer value.
    ///
    /// Returns `MZError::Param` on invalid values.
    pub fn new(flush: c_int) -> Result<Self, MZError> {
        match flush {
            0 => Ok(MZFlush::None),
            1 | 2 => Ok(MZFlush::Sync),
            3 => Ok(MZFlush::Full),
            4 => Ok(MZFlush::Finish),
            _ => Err(MZError::Param),
        }
    }
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MZStatus {
    Ok = 0,
    StreamEnd = 1,
    NeedDict = 2,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MZError {
    ErrNo = -1,
    Stream = -2,
    Data = -3,
    Mem = -4,
    Buf = -5,
    Version = -6,
    Param = -10_000,
}

pub type MZResult = Result<MZStatus, MZError>;
