/// Module that contains most of the functions exported to C.
use std::{slice, ptr};
use std::marker::PhantomData;

pub use tinfl::{tinfl_decompress, tinfl_decompress_mem_to_heap,
                tinfl_decompress_mem_to_mem, tinfl_decompressor};


pub use tdef::{tdefl_compress, tdefl_compress_buffer, tdefl_compress_mem_to_heap,
               tdefl_compress_mem_to_mem, tdefl_compress_mem_to_output,
               tdefl_create_comp_flags_from_zip_params, tdefl_get_prev_return_status, tdefl_init,
               tdefl_allocate, tdefl_deallocate, tdefl_get_adler32};
use libc::*;

use lib_oxide::{MZ_ADLER32_INIT, StreamOxide, StateType, InternalState, StateTypeEnum};

use miniz_oxide::{mz_adler32_oxide, MZError};

pub mod return_status {
    use MZError::*;
    use miniz_oxide::MZStatus;
    use libc::c_int;
    pub const MZ_ERRNO: c_int = ErrNo as c_int;
    pub const MZ_STREAM_ERROR: c_int = Stream as c_int;
    pub const MZ_DATA_ERROR: c_int = Data as c_int;
    pub const MZ_BUF_ERROR: c_int = Buf as c_int;
    pub const MZ_VERSION_ERROR: c_int = Version as c_int;
    pub const MZ_PARAM_ERROR: c_int = Param as c_int;

    pub const MZ_OK: c_int = MZStatus::Ok as c_int;
    pub const MZ_STREAM_END: c_int = MZStatus::StreamEnd as c_int;
    pub const MZ_NEED_DICT: c_int = MZStatus::NeedDict as c_int;
}

pub use return_status::*;

/// Deflate flush modes.
pub mod flush_modes {
    use libc::c_int;
    use miniz_oxide::deflate::core::TDEFLFlush;
    pub const MZ_NO_FLUSH: c_int = TDEFLFlush::None as c_int;
    // TODO: This is simply sync flush for now, miniz also treats it as such.
    pub const MZ_PARTIAL_FLUSH: c_int = 1;
    pub const MZ_SYNC_FLUSH: c_int = TDEFLFlush::Sync as c_int;
    pub const MZ_FULL_FLUSH: c_int = TDEFLFlush::Full as c_int;
    pub const MZ_FINISH: c_int = TDEFLFlush::Finish as c_int;
    // TODO: Doesn't seem to be implemented by miniz.
    pub const MZ_BLOCK: c_int = 5;
}

pub use flush_modes::*;

pub mod strategy {
    use libc::c_int;
    use miniz_oxide::deflate::core::CompressionStrategy::*;
    pub const MZ_DEFAULT_STRATEGY: c_int = Default as c_int;
    pub const MZ_FILTERED: c_int = Filtered as c_int;
    pub const MZ_HUFFMAN_ONLY: c_int = HuffmanOnly as c_int;
    pub const MZ_RLE: c_int = RLE as c_int;
    pub const MZ_FIXED: c_int = Fixed as c_int;
}

pub use strategy::*;

pub const MZ_CRC32_INIT: c_ulong = 0;

pub fn mz_crc32_oxide(crc32: c_uint, data: &[u8]) -> c_uint {
    let mut digest = crc32fast::Hasher::new_with_initial(crc32);
    digest.update(data);
    digest.finalize()
}

/// Signature of function used to allocate the compressor/decompressor structs.
#[allow(bad_style)]
pub type mz_alloc_func = unsafe extern "C" fn(*mut c_void, size_t, size_t) -> *mut c_void;
/// Signature of function used to free the compressor/decompressor structs.
#[allow(bad_style)]
pub type mz_free_func = unsafe extern "C" fn(*mut c_void, *mut c_void);

/// Inner stream state containing pointers to the used buffers and internal state.
#[repr(C)]
#[allow(bad_style)]
#[derive(Debug)]
pub struct mz_stream {
    /// Pointer to the current start of the input buffer.
    pub next_in: *const u8,
    /// Length of the input buffer.
    pub avail_in: c_uint,
    /// The total number of input bytes consumed so far.
    pub total_in: c_ulong,

    /// Pointer to the current start of the output buffer.
    pub next_out: *mut u8,
    /// Space in the output buffer.
    pub avail_out: c_uint,
    /// The total number of bytes output so far.
    pub total_out: c_ulong,

    pub msg: *const c_char,
    /// Compressor or decompressor, if it exists.
    /// This is boxed to work with the current C API.
    pub state: Option<Box<InternalState>>,

    /// Allocation function to use for allocating the internal compressor/decompressor.
    /// Uses `mz_default_alloc_func` if set to `None`.
    pub zalloc: Option<mz_alloc_func>,
    /// Free function to use for allocating the internal compressor/decompressor.
    /// Uses `mz_default_free_func` if `None`.
    pub zfree: Option<mz_free_func>,
    /// Extra data to provide the allocation/deallocation functions.
    /// (Not used for the default ones)
    pub opaque: *mut c_void,

    /// Whether the stream contains a compressor or decompressor.
    pub data_type: StateTypeEnum,
    /// Adler32 checksum of the data that has been compressed or uncompressed.
    pub adler: c_ulong,
    /// Reserved
    pub reserved: c_ulong,
}

impl Default for mz_stream {
    fn default() -> mz_stream {
        mz_stream {
            next_in: ptr::null(),
            avail_in: 0,
            total_in: 0,

            next_out: ptr::null_mut(),
            avail_out: 0,
            total_out: 0,

            msg: ptr::null(),
            state: None,

            zalloc: None,
            zfree: None,
            opaque: ptr::null_mut(),

            data_type: StateTypeEnum::None,
            adler: 0,
            reserved: 0,
        }
    }
}

impl<'io, ST: StateType> StreamOxide<'io, ST> {
    pub fn into_mz_stream(mut self) -> mz_stream {
        mz_stream {
            next_in: self.next_in.map_or(
                ptr::null(),
                |in_slice| in_slice.as_ptr(),
            ),
            avail_in: self.next_in.map_or(0, |in_slice| in_slice.len() as c_uint),
            total_in: self.total_in,

            next_out: self.next_out.as_mut().map_or(
                ptr::null_mut(),
                |out_slice| out_slice.as_mut_ptr(),
            ),
            avail_out: self.next_out.as_mut().map_or(
                0,
                |out_slice| out_slice.len() as c_uint,
            ),
            total_out: self.total_out,

            msg: ptr::null(),

            zalloc: None,
            zfree: None,
            opaque: ptr::null_mut(),
            state: self.state.take(),

            data_type: ST::STATE_TYPE,
            adler: self.adler as c_ulong,
            reserved: 0,
        }
    }

        /// Create a new StreamOxide wrapper from a [mz_stream] object.
    /// Custom allocation functions are not supported, supplying an mz_stream with allocation
    /// function will cause creation to fail.
    ///
    /// Unsafe as the mz_stream object is not guaranteed to be valid. It is up to the
    /// caller to ensure it is.
    pub unsafe fn new(stream: &mut mz_stream) -> Self {
        Self::try_new(stream)
            .expect("Failed to create StreamOxide, wrong state type or tried to specify allocators.")
    }

    /// Try to create a new StreamOxide wrapper from a [mz_stream] object.
    /// Custom allocation functions are not supported, supplying an mz_stream with allocation
    /// functions will cause creation to fail.
    ///
    /// Unsafe as the mz_stream object is not guaranteed to be valid. It is up to the
    /// caller to ensure it is.
    pub unsafe fn try_new(stream: &mut mz_stream) -> Result<Self, MZError> {
        // Make sure we don't make an inflate stream from a deflate stream and vice versa.
        if stream.data_type != ST::STATE_TYPE
            || stream.zalloc.is_some() || stream.zfree.is_some() {
            return Err(MZError::Param);
        }

        let in_slice = stream.next_in.as_ref().map(|ptr| {
            slice::from_raw_parts(ptr, stream.avail_in as usize)
        });

        let out_slice = stream.next_out.as_mut().map(|ptr| {
            slice::from_raw_parts_mut(ptr, stream.avail_out as usize)
        });

        Ok(StreamOxide {
            next_in: in_slice,
            total_in: stream.total_in,
            next_out: out_slice,
            total_out: stream.total_out,
            state: stream.state.take(),
            adler: stream.adler as u32,
            state_type: PhantomData,
        })
    }
}

#[cfg(not(no_c_export))]
unmangle!(
    /// Default allocation function using `malloc`.
    pub unsafe extern "C" fn miniz_def_alloc_func(
        _opaque: *mut c_void,
        items: size_t,
        size: size_t,
    ) -> *mut c_void {
        libc::malloc(items * size)
    }

    /// Default free function using `free`.
    pub unsafe extern "C" fn miniz_def_free_func(_opaque: *mut c_void, address: *mut c_void) {
        libc::free(address)
    }

    pub unsafe extern "C" fn miniz_def_realloc_func(
        _opaque: *mut c_void,
        address: *mut c_void,
        items: size_t,
        size: size_t,
    ) -> *mut c_void {
        libc::realloc(address, items * size)
    }

    /// Calculate adler32 checksum of the provided buffer with the initial adler32 checksum of `adler`.
    /// If c_ulong is wider than 32 bits, only the lower 32 bits will be used.
    ///
    /// Returns MZ_ADLER32_INIT if ptr is `ptr::null`.
    pub unsafe extern "C" fn mz_adler32(adler: c_ulong, ptr: *const u8, buf_len: usize) -> c_ulong {
        ptr.as_ref().map_or(MZ_ADLER32_INIT as c_ulong, |r| {
            let data = slice::from_raw_parts(r, buf_len);
            mz_adler32_oxide(adler as u32, data) as c_ulong
        })
    }

    /// Calculate crc-32 of the provided buffer with the initial CRC32 checksum of `crc`.
    /// If c_ulong is wider than 32 bits, only the lower 32 bits will be used.
    ///
    /// Returns MZ_CRC32_INIT if ptr is `ptr::null`.
    pub unsafe extern "C" fn mz_crc32(crc: c_ulong, ptr: *const u8, buf_len: size_t) -> c_ulong {
        ptr.as_ref().map_or(MZ_CRC32_INIT, |r| {
            let data = slice::from_raw_parts(r, buf_len);
            mz_crc32_oxide(crc as u32, data) as c_ulong
        })
}
);
