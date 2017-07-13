#![allow(dead_code)]

extern crate libc;

mod tdef_oxide;
pub use self::tdef_oxide::tdefl_radix_sort_syms_oxide;

use self::libc::*;
use std::slice;
use std::mem;

#[allow(bad_style)]
pub type tdefl_put_buf_func_ptr = unsafe extern "C" fn(*const c_void, c_int, *mut c_void);

pub const TDEFL_STATUS_BAD_PARAM: c_int = -2;
pub const TDEFL_STATUS_PUT_BUF_FAILED: c_int = -1;
pub const TDEFL_STATUS_OKAY: c_int = 0;
pub const TDEFL_STATUS_DONE: c_int = 1;

pub const TDEFL_NO_FLUSH: c_int = 0;
pub const TDEFL_SYNC_FLUSH: c_int = 2;
pub const TDEFL_FULL_FLUSH: c_int = 3;
pub const TDEFL_FINISH: c_int = 4;

pub const TDEFL_LZ_CODE_BUF_SIZE: c_int = 64 * 1024;
pub const TDEFL_OUT_BUF_SIZE: c_int = (TDEFL_LZ_CODE_BUF_SIZE * 13) / 10;
pub const TDEFL_MAX_HUFF_SYMBOLS: c_int = 288;
pub const TDEFL_LZ_HASH_BITS: c_int = 15;
pub const TDEFL_LEVEL1_HASH_SIZE_MASK: c_int = 4095;
pub const TDEFL_LZ_HASH_SHIFT: c_int = (TDEFL_LZ_HASH_BITS + 2) / 3;
pub const TDEFL_LZ_HASH_SIZE: c_int = 1 << TDEFL_LZ_HASH_BITS;

pub const TDEFL_MAX_HUFF_TABLES: c_int = 3;
pub const TDEFL_MAX_HUFF_SYMBOLS_0: c_int = 288;
pub const TDEFL_MAX_HUFF_SYMBOLS_1: c_int = 32;
pub const TDEFL_MAX_HUFF_SYMBOLS_2: c_int = 19;
pub const TDEFL_LZ_DICT_SIZE: c_int = 32768;
pub const TDEFL_LZ_DICT_SIZE_MASK: c_int = TDEFL_LZ_DICT_SIZE - 1;
pub const TDEFL_MIN_MATCH_LEN: c_int = 3;
pub const TDEFL_MAX_MATCH_LEN: c_int = 258;

pub const TDEFL_WRITE_ZLIB_HEADER: c_int = 0x01000;
pub const TDEFL_COMPUTE_ADLER32: c_int = 0x02000;
pub const TDEFL_GREEDY_PARSING_FLAG: c_int = 0x04000;
pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: c_int = 0x08000;
pub const TDEFL_RLE_MATCHES: c_int = 0x10000;
pub const TDEFL_FILTER_MATCHES: c_int = 0x20000;
pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: c_int = 0x40000;
pub const TDEFL_FORCE_ALL_RAW_BLOCKS: c_int = 0x80000;

#[repr(C)]
#[allow(bad_style)]
pub struct tdefl_compressor {
    m_pPut_buf_func: tdefl_put_buf_func_ptr,
    m_pPut_buf_user: *mut c_void,

    m_flags: c_uint,
    m_max_probes: [c_uint; 2],

    m_greedy_parsing: c_int,

    m_adler32: c_uint,
    m_lookahead_pos: c_uint,
    m_lookahead_size: c_uint,
    m_dict_size: c_uint,

    m_pLZ_code_buf: *mut u8,
    m_pLZ_flags: *mut u8,
    m_pOutput_buf: *mut u8,
    m_pOutput_buf_end: *mut u8,

    m_num_flags_left: c_uint,
    m_total_lz_bytes: c_uint,
    m_lz_code_buf_dict_pos: c_uint,
    m_bits_in: c_uint,
    m_bit_buffer: c_uint,

    m_saved_match_dist: c_uint,
    m_saved_match_len: c_uint,
    m_saved_lit: c_uint,
    m_output_flush_ofs: c_uint,
    m_output_flush_remaining: c_uint,
    m_finished: c_uint,
    m_block_index: c_uint,
    m_wants_to_finish: c_uint,

    m_prev_return_status: c_int,

    m_pIn_buf: *const c_void,
    m_pOut_buf: *mut c_void,
    m_pIn_buf_size: *mut size_t,
    m_pOut_buf_size: *mut size_t,

    m_flush: c_int,

    m_pSrc: *const u8,

    m_src_buf_left: size_t,
    m_out_buf_ofs: size_t,

    m_dict: [u8; (TDEFL_LZ_DICT_SIZE + TDEFL_MAX_MATCH_LEN - 1) as usize],
    m_huff_count: [[u16; TDEFL_MAX_HUFF_SYMBOLS as usize]; TDEFL_MAX_HUFF_TABLES as usize],
    m_huff_codes: [[u16; TDEFL_MAX_HUFF_SYMBOLS as usize]; TDEFL_MAX_HUFF_TABLES as usize],
    m_huff_code_sizes: [[u8; TDEFL_MAX_HUFF_SYMBOLS as usize]; TDEFL_MAX_HUFF_TABLES as usize],
    m_lz_code_buf: [u8; TDEFL_LZ_CODE_BUF_SIZE as usize],
    m_next: [u16; TDEFL_LZ_DICT_SIZE as usize],
    m_hash: [u16; TDEFL_LZ_HASH_SIZE as usize],
    m_output_buf: [u8; TDEFL_OUT_BUF_SIZE as usize],
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
pub unsafe extern "C" fn tdefl_radix_sort_syms(num_syms : c_uint, pSyms0: *mut tdefl_sym_freq,
                                               pSyms1: *mut tdefl_sym_freq) -> *mut tdefl_sym_freq {
    let syms0 = slice::from_raw_parts_mut(pSyms0, num_syms as usize);
    let syms1 = slice::from_raw_parts_mut(pSyms1, num_syms as usize);
    tdefl_radix_sort_syms_oxide(syms0, syms1).as_mut_ptr()
}
