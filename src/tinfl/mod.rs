#![allow(dead_code)]

use ::libc::*;
use std::{mem, usize};

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
            code_size: [0;288],
            look_up: [0;1024],
            tree: [0; 576],
        }
    }
}

const TINFL_MAX_HUFF_TABLES: usize = 3;
const TINFL_MAX_HUFF_SYMBOLS_0: usize = 288;
const TINFL_MAX_HUFF_SYMBOLS_1: usize = 32;
const TINFL_MAX_HUFF_SYMBOLS_2: usize = 19;

pub const TINFL_FLAG_PARSE_ZLIB_HEADER: i32 = 1;
pub const TINFL_FLAG_HAS_MORE_INPUT: i32 = 2;
pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: i32 = 4;
pub const TINFL_FLAG_COMPUTE_ADLER32: i32 = 8;

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
    pub bit_buf: u32,
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
    ) -> c_int;
}

pub const TINFL_DECOMPRESS_MEM_TO_MEM_FAILED: size_t = usize::MAX;

#[no_mangle]
pub unsafe extern "C" fn tinfl_decompress_mem_to_mem(
    p_out_buf: *mut c_void,
    mut out_buf_len: size_t,
    p_src_buf: *mut c_void,
    mut src_buf_len: size_t,
    flags: c_int
) -> size_t
{
    let mut decomp = tinfl_decompressor::with_init_state_only();

    let status = tinfl_decompress(
        &mut decomp,
        p_src_buf as *const u8,
        &mut src_buf_len as *mut size_t,
        p_out_buf as *mut u8,
        p_out_buf as *mut u8,
        &mut out_buf_len as *mut size_t,
        // This function takes an unsigned value for flags, so we need to explicitly cast
        // the flags to u32.
        ((flags & !TINFL_FLAG_HAS_MORE_INPUT) | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF) as u32);

    if status != TINFL_STATUS_DONE {
        TINFL_DECOMPRESS_MEM_TO_MEM_FAILED as size_t
    } else {
        out_buf_len
    }
}



#[cfg(test)]
mod test {
    use super::*;
    use libc::c_void;
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
            flags,
        ).unwrap();
        assert_eq!(&out_buf[..size], &b"Hello, zlib!"[..]);
    }
}
