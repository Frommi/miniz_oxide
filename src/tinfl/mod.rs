#![allow(dead_code)]

use ::libc::*;

pub const TINFL_LZ_DICT_SIZE: usize = 32768;

#[repr(C)]
#[allow(bad_style)]
pub struct tinfl_huff_table {
    pub code_size: [u8; 288],
    pub look_up: [i16; 1024],
    pub tree: [i16; 576],
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
