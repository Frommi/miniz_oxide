#![allow(dead_code)]

extern crate libc;

mod tdef_oxide;
pub use self::tdef_oxide::*;

use self::libc::*;
use std::slice;
use std::mem;
use std::cmp;
use std::io;
use std::io::{Cursor, Write};

#[allow(bad_style)]
pub type tdefl_put_buf_func_ptr = Option<unsafe extern "C" fn(*const c_void, c_int, *mut c_void)>;

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TDEFLStatus {
    BadParam = -2,
    PutBufFailed = -1,
    Okay = 0,
    Done = 1
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TDEFLFlush {
    None = 0,
    Sync = 2,
    Full = 3,
    Finish = 4
}

pub const TDEFL_LZ_CODE_BUF_SIZE: usize = 64 * 1024;
pub const TDEFL_OUT_BUF_SIZE: usize = (TDEFL_LZ_CODE_BUF_SIZE * 13) / 10;
pub const TDEFL_MAX_HUFF_SYMBOLS: usize = 288;
pub const TDEFL_LZ_HASH_BITS: c_int = 15;
pub const TDEFL_LEVEL1_HASH_SIZE_MASK: c_int = 4095;
pub const TDEFL_LZ_HASH_SHIFT: c_int = (TDEFL_LZ_HASH_BITS + 2) / 3;
pub const TDEFL_LZ_HASH_SIZE: usize = 1 << TDEFL_LZ_HASH_BITS;

pub const TDEFL_MAX_HUFF_TABLES: usize = 3;
pub const TDEFL_MAX_HUFF_SYMBOLS_0: usize = 288;
pub const TDEFL_MAX_HUFF_SYMBOLS_1: usize = 32;
pub const TDEFL_MAX_HUFF_SYMBOLS_2: usize = 19;
pub const TDEFL_LZ_DICT_SIZE: usize = 32768;
pub const TDEFL_LZ_DICT_SIZE_MASK: c_uint = TDEFL_LZ_DICT_SIZE as c_uint - 1;
pub const TDEFL_MIN_MATCH_LEN: usize = 3;
pub const TDEFL_MAX_MATCH_LEN: usize = 258;

pub const TDEFL_WRITE_ZLIB_HEADER: c_int = 0x01000;
pub const TDEFL_COMPUTE_ADLER32: c_int = 0x02000;
pub const TDEFL_GREEDY_PARSING_FLAG: c_int = 0x04000;
pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: c_int = 0x08000;
pub const TDEFL_RLE_MATCHES: c_int = 0x10000;
pub const TDEFL_FILTER_MATCHES: c_int = 0x20000;
pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: c_int = 0x40000;
pub const TDEFL_FORCE_ALL_RAW_BLOCKS: c_int = 0x80000;

pub const TDEFL_HUFFMAN_ONLY: c_int = 0;
pub const TDEFL_DEFAULT_MAX_PROBES: c_int = 128;
pub const TDEFL_MAX_PROBES_MASK: c_int = 0xFFF;

const TDEFL_MAX_SUPPORTED_HUFF_CODESIZE: usize = 32;

#[repr(C)]
#[allow(bad_style)]
pub struct tdefl_compressor {
    pub m_pPut_buf_func: tdefl_put_buf_func_ptr,
    pub m_pPut_buf_user: *mut c_void,

    pub m_flags: c_uint,
    pub m_max_probes: [c_uint; 2],

    pub m_greedy_parsing: c_int,

    pub m_adler32: c_uint,
    pub m_lookahead_pos: c_uint,
    pub m_lookahead_size: c_uint,
    pub m_dict_size: c_uint,

    pub m_pLZ_code_buf: *mut u8,
    pub m_pLZ_flags: *mut u8,
    pub m_pOutput_buf: *mut u8,                  // current output buffer
    pub m_pOutput_buf_end: *mut u8,

    pub m_num_flags_left: c_uint,
    pub m_total_lz_bytes: c_uint,
    pub m_lz_code_buf_dict_pos: c_uint,
    pub m_bits_in: c_uint,
    pub m_bit_buffer: c_uint,

    pub m_saved_match_dist: c_uint,
    pub m_saved_match_len: c_uint,
    pub m_saved_lit: c_uint,
    pub m_output_flush_ofs: c_uint,
    pub m_output_flush_remaining: c_uint,
    pub m_finished: c_uint,
    pub m_block_index: c_uint,
    pub m_wants_to_finish: c_uint,

    pub m_prev_return_status: TDEFLStatus,

    pub m_pIn_buf: *const c_void,
    pub m_pOut_buf: *mut c_void,                 // original out_buf from tdefl_compress
    pub m_pIn_buf_size: *mut usize,
    pub m_pOut_buf_size: *mut usize,

    pub m_flush: TDEFLFlush,

    pub m_pSrc: *const u8,
    pub m_src_buf_left: usize,
    pub m_out_buf_ofs: usize,

    pub m_dict: [u8; TDEFL_LZ_DICT_SIZE + TDEFL_MAX_MATCH_LEN - 1],
    pub m_huff_count: [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub m_huff_codes: [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub m_huff_code_sizes: [[u8; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub m_lz_code_buf: [u8; TDEFL_LZ_CODE_BUF_SIZE],
    pub m_next: [u16; TDEFL_LZ_DICT_SIZE],
    pub m_hash: [u16; TDEFL_LZ_HASH_SIZE],
    pub m_output_buf: [u8; TDEFL_OUT_BUF_SIZE],  // local output buffer
}

const TDEFL_LEN_SYM: [u16; 256] = [
    257, 258, 259, 260, 261, 262, 263, 264, 265, 265, 266, 266, 267, 267, 268, 268, 269, 269, 269, 269, 270, 270, 270, 270, 271, 271, 271, 271, 272, 272, 272, 272,
    273, 273, 273, 273, 273, 273, 273, 273, 274, 274, 274, 274, 274, 274, 274, 274, 275, 275, 275, 275, 275, 275, 275, 275, 276, 276, 276, 276, 276, 276, 276, 276,
    277, 277, 277, 277, 277, 277, 277, 277, 277, 277, 277, 277, 277, 277, 277, 277, 278, 278, 278, 278, 278, 278, 278, 278, 278, 278, 278, 278, 278, 278, 278, 278,
    279, 279, 279, 279, 279, 279, 279, 279, 279, 279, 279, 279, 279, 279, 279, 279, 280, 280, 280, 280, 280, 280, 280, 280, 280, 280, 280, 280, 280, 280, 280, 280,
    281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281, 281,
    282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282, 282,
    283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283, 283,
    284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 284, 285
];

const TDEFL_LEN_EXTRA: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 0
];

const TDEFL_SMALL_DIST_SYM: [u8; 512] = [
    0, 1, 2, 3, 4, 4, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14,
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14,
    14, 14, 14, 14, 14, 14, 14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17
];

const TDEFL_SMALL_DIST_EXTRA: [u8; 512] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7
];

const TDEFL_LARGE_DIST_SYM: [u8; 128] = [
    0, 0, 18, 19, 20, 20, 21, 21, 22, 22, 22, 22, 23, 23, 23, 23, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 25, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26,
    26, 26, 26, 26, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29
];

const TDEFL_LARGE_DIST_EXTRA: [u8; 128] = [
    0, 0, 8, 8, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12,
    12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13
];

const MZ_BITMASKS: [u32; 17] = [
    0x0000, 0x0001, 0x0003, 0x0007, 0x000F, 0x001F, 0x003F, 0x007F, 0x00FF,
    0x01FF, 0x03FF, 0x07FF, 0x0FFF, 0x1FFF, 0x3FFF, 0x7FFF, 0xFFFF
];

const TDEFL_NUM_PROBES: [c_uint; 11] = [0, 1, 6, 32, 16, 32, 128, 256, 512, 768, 1500];

#[allow(bad_style)]
extern {
    pub fn tdefl_init(d: *mut tdefl_compressor,
                      pPut_buf_func: tdefl_put_buf_func_ptr,
                      pPut_buf_user: *mut c_void,
                      flags: c_int) -> c_int;

    pub fn tdefl_compress(d: *mut tdefl_compressor,
                          pIn_buf: *const c_void,
                          pIn_buf_size: *mut size_t,
                          pOut_buf: *mut c_void,
                          pOut_buf_size: *mut size_t,
                          flush: c_int) -> c_int;
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(bad_style)]
pub struct tdefl_sym_freq {
    m_key: u16,
    m_sym_index: u16,
}

#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn tdefl_radix_sort_syms(num_syms : c_uint,
                                               pSyms0: *mut tdefl_sym_freq,
                                               pSyms1: *mut tdefl_sym_freq) -> *mut tdefl_sym_freq
{
    let syms0 = slice::from_raw_parts_mut(pSyms0, num_syms as usize);
    let syms1 = slice::from_raw_parts_mut(pSyms1, num_syms as usize);
    tdefl_radix_sort_syms_oxide(syms0, syms1).as_mut_ptr()
}

#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn tdefl_calculate_minimum_redundancy(A: *mut tdefl_sym_freq, n: c_int) {
    let symbols = slice::from_raw_parts_mut(A, n as usize);
    tdefl_calculate_minimum_redundancy_oxide(symbols)
}

#[no_mangle]
#[allow(bad_style)]
pub unsafe extern "C" fn tdefl_huffman_enforce_max_code_size(pNum_codes: *mut c_int,
                                                             code_list_len: c_int,
                                                             max_code_size: c_int)
{
    let num_codes = slice::from_raw_parts_mut(pNum_codes, TDEFL_MAX_SUPPORTED_HUFF_CODESIZE + 1);
    tdefl_huffman_enforce_max_code_size_oxide(num_codes, code_list_len as usize, max_code_size as usize)
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_optimize_huffman_table(d: *mut tdefl_compressor,
                                                      table_num: c_int,
                                                      table_len: c_int,
                                                      code_size_limit: c_int,
                                                      static_table: c_int)
{
    tdefl_optimize_huffman_table_oxide(&mut HuffmanOxide::new(d),
                                       table_num as usize,
                                       table_len as usize,
                                       code_size_limit as usize,
                                       static_table != 0)
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_start_dynamic_block(d: *mut tdefl_compressor) {
    let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");
    let mut ob = OutputBufferOxide::new(d);
    tdefl_start_dynamic_block_oxide(&mut HuffmanOxide::new(d), &mut ob)
        .expect("io error in tdefl_start_dynamic_block_oxide");

    (*d).m_pOutput_buf = (*d).m_pOutput_buf.offset(ob.inner.position() as isize);
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_start_static_block(d: *mut tdefl_compressor) {
    let mut ob = OutputBufferOxide::new(d);
    tdefl_start_static_block_oxide(&mut HuffmanOxide::new(d), &mut ob)
        .expect("io error in tdefl_start_static_block_oxide");

    (*d).m_pOutput_buf = (*d).m_pOutput_buf.offset(ob.inner.position() as isize);
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_lz_codes(d: *mut tdefl_compressor) -> bool {
    let mut ob = OutputBufferOxide::new(d);
    let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");

    let len = d.m_pLZ_code_buf as usize - (&d.m_lz_code_buf as *const [u8; TDEFL_LZ_CODE_BUF_SIZE]) as usize;
    let lz_code_buf = slice::from_raw_parts(&d.m_lz_code_buf[0] as *const u8, len);

    let res = match tdefl_compress_lz_codes_oxide(&mut HuffmanOxide::new(d), &mut ob, &lz_code_buf) {
        Err(_) => false,
        Ok(b) => b
    };

    d.m_pOutput_buf = d.m_pOutput_buf.offset(ob.inner.position() as isize);

    res && (d.m_pOutput_buf < d.m_pOutput_buf_end)
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_block(d: *mut tdefl_compressor, static_block: bool) -> bool {
    let mut ob = OutputBufferOxide::new(d);

    let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");
    let len = d.m_pLZ_code_buf as usize - (&d.m_lz_code_buf as *const [u8; TDEFL_LZ_CODE_BUF_SIZE]) as usize;
    let lz_code_buf = slice::from_raw_parts(&d.m_lz_code_buf[0] as *const u8, len);

    let res = match tdefl_compress_block_oxide(&mut HuffmanOxide::new(d), &mut ob, lz_code_buf, static_block) {
        Err(_) => false,
        Ok(b) => b
    };

    d.m_pOutput_buf = d.m_pOutput_buf.offset(ob.inner.position() as isize);

    res
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_find_match(d: *mut tdefl_compressor,
                                          lookahead_pos: c_uint,
                                          max_dist: c_uint,
                                          max_match_len: c_uint,
                                          match_dist: &mut c_uint,
                                          match_len: &mut c_uint)
{
    let dist_len = tdefl_find_match_oxide(
        &mut DictOxide::new(d),
        lookahead_pos,
        max_dist,
        max_match_len,
        *match_dist,
        *match_len
    );

    *match_dist = dist_len.0;
    *match_len = dist_len.1;
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_record_literal(d: *mut tdefl_compressor, lit: u8) {
    let mut lz = LZOxide::new(d);
    tdefl_record_literal_oxide(&mut HuffmanOxide::new(d), &mut lz, lit);

    let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");
    d.m_pLZ_code_buf = (&mut lz.codes[0] as *mut u8).offset(lz.code_position as isize);
    d.m_pLZ_flags = (&mut lz.codes[0] as *mut u8).offset(lz.flag_position as isize);
    d.m_total_lz_bytes = lz.total_bytes;
    d.m_num_flags_left = lz.num_flags_left;
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_record_match(d: *mut tdefl_compressor,
                                            match_len: c_uint,
                                            match_dist: c_uint)
{
    let mut lz = LZOxide::new(d);
    tdefl_record_match_oxide(&mut HuffmanOxide::new(d), &mut lz, match_len, match_dist);

    let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");
    d.m_pLZ_code_buf = (&mut lz.codes[0] as *mut u8).offset(lz.code_position as isize);
    d.m_pLZ_flags = (&mut lz.codes[0] as *mut u8).offset(lz.flag_position as isize);
    d.m_total_lz_bytes = lz.total_bytes;
    d.m_num_flags_left = lz.num_flags_left;
}

#[no_mangle]
pub extern "C" fn tdefl_create_comp_flags_from_zip_params(level: c_int,
                                                          window_bits: c_int,
                                                          strategy: c_int) -> c_uint
{
    tdefl_create_comp_flags_from_zip_params_oxide(level, window_bits, strategy)
}
