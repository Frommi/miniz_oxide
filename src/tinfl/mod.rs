#![allow(dead_code)]

use ::libc::*;
use std::{slice, mem, ptr, usize};
use std::io::Cursor;

mod tinfl_oxide;
pub use self::tinfl_oxide::*;

pub const TINFL_LZ_DICT_SIZE: usize = 32768;

#[repr(C)]
#[allow(bad_style)]
pub struct tinfl_huff_table {
    pub code_size: [u8; 288],
    pub look_up: [i16; 1024],
    pub tree: [i16; 576],
}

impl tinfl_huff_table {
    fn new() -> tinfl_huff_table {
        tinfl_huff_table {
            code_size: [0; 288],
            look_up: [0; 1024],
            tree: [0; 576],
        }
    }
}

const TINFL_MAX_HUFF_TABLES: usize = 3;
const TINFL_MAX_HUFF_SYMBOLS_0: usize = 288;
const TINFL_MAX_HUFF_SYMBOLS_1: usize = 32;
const TINFL_MAX_HUFF_SYMBOLS_2: usize = 19;
const TINFL_FAST_LOOKUP_BITS: u8 = 10;
const TINFL_FAST_LOOKUP_SIZE: u32 = 1 << TINFL_FAST_LOOKUP_BITS;

pub const TINFL_FLAG_PARSE_ZLIB_HEADER: u32 = 1;
pub const TINFL_FLAG_HAS_MORE_INPUT: u32 = 2;
pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: u32 = 4;
pub const TINFL_FLAG_COMPUTE_ADLER32: u32 = 8;

pub const TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS: i32 = -4;
pub const TINFL_STATUS_BAD_PARAM: i32 = -3;
pub const TINFL_STATUS_ADLER32_MISMATCH: i32 = -2;
pub const TINFL_STATUS_FAILED: i32 = -1;
pub const TINFL_STATUS_DONE: i32 = 0;
pub const TINFL_STATUS_NEEDS_MORE_INPUT: i32 = 1;
pub const TINFL_STATUS_HAS_MORE_OUTPUT: i32 = 2;

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TINFLStatus {
    FailedCannotMakeProgress = TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS,
    BadParam = TINFL_STATUS_BAD_PARAM,
    Adler32Mismatch = TINFL_STATUS_ADLER32_MISMATCH,
    Failed = TINFL_STATUS_FAILED,
    Done = TINFL_STATUS_DONE,
    NeedsMoreInput = TINFL_STATUS_NEEDS_MORE_INPUT,
    HasMoreOutput = TINFL_STATUS_HAS_MORE_OUTPUT,
}

pub const TDEFL_WRITE_ZLIB_HEADER: u32 = 0x01000;
pub const TDEFL_COMPUTE_ADLER32: u32 = 0x02000;
pub const TDEFL_GREEDY_PARSING_FLAG: u32 = 0x04000;
pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: u32 = 0x08000;
pub const TDEFL_RLE_MATCHES: u32 = 0x10000;
pub const TDEFL_FILTER_MATCHES: u32 = 0x20000;
pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: u32 = 0x40000;
pub const TDEFL_FORCE_ALL_RAW_BLOCKS: u32 = 0x80000;

type BitBuffer = u32;

#[repr(C)]
#[allow(bad_style)]
pub struct tinfl_decompressor {
    pub state: u32,
    pub num_bits: u32,
    pub z_header0: u32,
    pub z_header1: u32,
    pub z_adler32: u32,
    pub finish: u32,
    pub block_type: u32,
    pub check_adler32: u32,
    pub dist: u32,
    pub counter: u32,
    pub num_extra: u32,
    pub table_sizes: [u32; TINFL_MAX_HUFF_TABLES],
    pub bit_buf: BitBuffer,
    pub dist_from_out_buf_start: usize,
    pub tables: [tinfl_huff_table; TINFL_MAX_HUFF_TABLES],
    pub raw_header: [u8; 4],
    pub len_codes: [u8; TINFL_MAX_HUFF_SYMBOLS_0 + TINFL_MAX_HUFF_SYMBOLS_1 + 137],
}

impl tinfl_decompressor {
    /// Create a new tinfl_decompressor with all fields set to 0.
    pub fn new() -> tinfl_decompressor {
        tinfl_decompressor {
            state: 0,
            num_bits: 0,
            z_header0: 0,
            z_header1: 0,
            z_adler32: 0,
            finish: 0,
            block_type: 0,
            check_adler32: 0,
            dist: 0,
            counter: 0,
            num_extra: 0,
            table_sizes: [0; TINFL_MAX_HUFF_TABLES],
            bit_buf: 0,
            dist_from_out_buf_start: 0,
            // TODO:(oyvindln) Check that copies here are optimized out in release mode.
            tables: [tinfl_huff_table::new(), tinfl_huff_table::new(), tinfl_huff_table::new()],
            raw_header: [0; 4],
            len_codes: [0; TINFL_MAX_HUFF_SYMBOLS_0 + TINFL_MAX_HUFF_SYMBOLS_1 + 137],
        }
    }

    /// Create a new decompressor with only the state field initialized.
    ///
    /// This is how it's created in miniz.
    pub unsafe fn with_init_state_only() -> tinfl_decompressor {
        let mut decomp: tinfl_decompressor = mem::uninitialized();
        decomp.state = 0;
        decomp
    }
}

#[allow(bad_style)]
extern {
    pub fn tinfl_decompress(
        r: *mut tinfl_decompressor,
        pIn_buf_next: *const u8,
        pIn_buf_size: *mut size_t,
        pOut_buf_start: *mut u8,
        pOut_buf_next: *mut u8,
        pOut_buf_size: *mut size_t,
        decomp_flags: c_uint
    ) -> TINFLStatus;
}

pub const TINFL_DECOMPRESS_MEM_TO_MEM_FAILED: size_t = usize::MAX;

#[no_mangle]
pub unsafe extern "C" fn tinfl_decompress_mem_to_mem(
    p_out_buf: *mut c_void,
    out_buf_len: size_t,
    p_src_buf: *const c_void,
    src_buf_len: size_t,
    flags: c_int,
) -> size_t {
    let flags = flags as u32;
    let mut decomp = tinfl_decompressor::with_init_state_only();

    let (status, _, out_consumed) = decompress_oxide(
        &mut decomp,
        slice::from_raw_parts(p_src_buf as *const u8, src_buf_len),
        &mut Cursor::new(slice::from_raw_parts_mut(
            p_out_buf as *mut u8,
            out_buf_len
        )),
        ((flags & !TINFL_FLAG_HAS_MORE_INPUT) | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF),
    );

    if status != TINFLStatus::Done {
        TINFL_DECOMPRESS_MEM_TO_MEM_FAILED as size_t
    } else {
        out_consumed
    }
}

#[no_mangle]
/// Decompress data from p_src_buf to a continiuosly growing heap-allocated buffer.
///
/// Sets `p_out_len` to the length of the returned buffer.
/// Returns `ptr::null()` if decompression or allocation fails.
/// The buffer should be freed with `miniz_def_free_func`.
pub unsafe extern "C" fn tinfl_decompress_mem_to_heap(
    p_src_buf: *const c_void,
    src_buf_len: size_t,
    p_out_len: *mut size_t,
    flags: c_int,
) -> *mut c_void {
    let flags = flags as u32;
    const MIN_BUFFER_CAPACITY: size_t = 128;

    // We're not using a Vec for the buffer here to make sure the buffer is allocated and freed by
    // the same allocator.

    let mut decomp = tinfl_decompressor::with_init_state_only();
    // Pointer to the buffer to place the decompressed data into.
    let mut p_buf: *mut c_void = ptr::null_mut();
    // Capacity of the current output buffer.
    let mut out_buf_capacity = 0;
    //let p_new_buf;
    *p_out_len = 0;
    // How far into the source buffer we have read.
    let mut src_buf_ofs = 0;
    loop {
        let mut out_cur = Cursor::new(slice::from_raw_parts_mut(
            p_buf as *mut u8,
            out_buf_capacity
        ));
        out_cur.set_position(*p_out_len as u64);
        let (status, in_consumed, out_consumed) = decompress_oxide(
            &mut decomp,
            slice::from_raw_parts(
                p_src_buf.offset(src_buf_ofs as isize) as *const u8,
                src_buf_len - src_buf_ofs,
            ),
            &mut out_cur,
            ((flags & !TINFL_FLAG_HAS_MORE_INPUT) | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF),
        );

        // If decompression fails or we don't have any input, bail out.
        if (status as i32) < 0 || status == TINFLStatus::NeedsMoreInput {
            ::miniz_def_free_func(ptr::null_mut(), p_buf);
            *p_out_len = 0;
            return ptr::null_mut();
        }

        src_buf_ofs += in_consumed;
        *p_out_len += out_consumed;

        if status == TINFLStatus::Done {
            break;
        }

        // If we need more space, double the capacity of the output buffer
        // and keep going.
        let mut new_out_buf_capacity = out_buf_capacity * 2;

        // Try to get at least 128 bytes of buffer capacity.
        if new_out_buf_capacity < MIN_BUFFER_CAPACITY {
            new_out_buf_capacity = MIN_BUFFER_CAPACITY
        }

        let p_new_buf = ::miniz_def_realloc_func(
            ptr::null_mut(),
            p_buf,
            1,
            new_out_buf_capacity
        );
        // Bail out if growing fails.
        if p_new_buf.is_null() {
            ::miniz_def_free_func(ptr::null_mut(), p_buf);
            *p_out_len = 0;
            return ptr::null_mut();
        }

        // Otherwise, continue using the reallocated buffer.
        p_buf = p_new_buf;
        out_buf_capacity = new_out_buf_capacity;
    }

    p_buf
}

#[cfg(test)]
mod test {
    use super::*;
    use libc::c_void;
    use std::{ops, slice};
    /// Safe wrapper for `tinfl_decompress_mem_to_mem` using slices.
    ///
    /// Could maybe make this public later.
    fn tinfl_decompress_mem_to_mem_wrapper(source: &mut [u8], dest: &mut [u8], flags: i32) -> Option<usize> {
        let status = unsafe {
            let source_len = source.len();
            let dest_len = dest.len();
            tinfl_decompress_mem_to_mem(
                dest.as_mut_ptr() as *mut c_void,
                dest_len,
                source.as_mut_ptr() as *mut c_void,
                source_len,
                flags
            )
        };
        if status != TINFL_DECOMPRESS_MEM_TO_MEM_FAILED {
            Some(status)
        } else {
            None
        }
    }

    /// Safe wrapper around a buffer allocated with the miniz_def functions.
    pub struct TinflHeapBuf {
        buf: *mut c_void,
        len: size_t,
    }

    impl TinflHeapBuf {
        fn as_slice(&self) -> &[u8] {
            unsafe {
                slice::from_raw_parts(self.buf as *const u8, self.len)
            }
        }
    }

    impl ops::Drop for TinflHeapBuf {
        fn drop(&mut self) {
            unsafe {
                ::miniz_def_free_func(ptr::null_mut(), self.buf);
            }
        }
    }

    /// Safe wrapper for `tinfl_decompress_mem_to_heap` using slices.
    ///
    /// Could maybe make something like this public later.
    fn tinfl_decompress_mem_to_heap_wrapper(source: &mut [u8], flags: i32) -> Option<TinflHeapBuf> {
        let source_len = source.len();
        let mut out_len = 0;
        unsafe {
            let buf_ptr = tinfl_decompress_mem_to_heap(
                source.as_ptr() as *const c_void,
                source_len,
                &mut out_len,
                flags,
            );
            if !buf_ptr.is_null() {
                Some(TinflHeapBuf {
                    buf: buf_ptr,
                    len: out_len,
                })
            } else {
                None
            }
        }
    }

    #[test]
    fn mem_to_mem() {
        let mut encoded =
            [120, 156, 243, 72, 205, 201, 201, 215, 81,
             168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19];
        let mut out_buf = vec![0;50];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;
        let size = tinfl_decompress_mem_to_mem_wrapper(
            &mut encoded[..],
            out_buf.as_mut_slice(),
            flags as i32,
        ).unwrap();
        assert_eq!(&out_buf[..size], &b"Hello, zlib!"[..]);
    }

    #[test]
    fn mem_to_heap() {
        let mut encoded =
            [120, 156, 243, 72, 205, 201, 201, 215, 81,
             168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;
        let out_buf = tinfl_decompress_mem_to_heap_wrapper(
            &mut encoded[..],
            flags as i32,
        ).unwrap();
        assert_eq!(out_buf.as_slice(), &b"Hello, zlib!"[..]);
    }
}
