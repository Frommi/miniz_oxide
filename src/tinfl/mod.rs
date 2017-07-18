extern crate libc;

use self::libc::*;

pub const TINFL_LZ_DICT_SIZE: usize = 32768;

#[repr(C)]
#[allow(bad_style)]
pub struct tinfl_huff_table {
    pub m_code_size: [u8; 288usize],
    pub m_look_up: [c_short; 1024usize],
    pub m_tree: [c_short; 576usize],
}

const TINFL_MAX_HUFF_TABLES: usize = 3;
const TINFL_MAX_HUFF_SYMBOLS_0: usize = 288;
const TINFL_MAX_HUFF_SYMBOLS_1: usize = 32;
const TINFL_MAX_HUFF_SYMBOLS_2: usize = 19;

pub const TINFL_FLAG_PARSE_ZLIB_HEADER: c_int = 1;
pub const TINFL_FLAG_HAS_MORE_INPUT: c_int = 2;
pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: c_int = 4;
pub const TINFL_FLAG_COMPUTE_ADLER32: c_int = 8;

pub const TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS: c_int = -4;
pub const TINFL_STATUS_BAD_PARAM: c_int = -3;
pub const TINFL_STATUS_ADLER32_MISMATCH: c_int = -2;
pub const TINFL_STATUS_FAILED: c_int = -1;
pub const TINFL_STATUS_DONE: c_int = 0;
pub const TINFL_STATUS_NEEDS_MORE_INPUT: c_int = 1;
pub const TINFL_STATUS_HAS_MORE_OUTPUT: c_int = 2;

pub const TDEFL_WRITE_ZLIB_HEADER: c_uint = 0x01000;
pub const TDEFL_COMPUTE_ADLER32: c_uint = 0x02000;
pub const TDEFL_GREEDY_PARSING_FLAG: c_uint = 0x04000;
pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: c_uint = 0x08000;
pub const TDEFL_RLE_MATCHES: c_uint = 0x10000;
pub const TDEFL_FILTER_MATCHES: c_uint = 0x20000;
pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: c_uint = 0x40000;
pub const TDEFL_FORCE_ALL_RAW_BLOCKS: c_uint = 0x80000;

#[repr(C)]
#[allow(bad_style)]
pub struct tinfl_decompressor {
    pub m_state: c_uint,
    pub m_num_bits: c_uint,
    pub m_zhdr0: c_uint,
    pub m_zhdr1: c_uint,
    pub m_z_adler32: c_uint,
    pub m_final: c_uint,
    pub m_type: c_uint,
    pub m_check_adler32: c_uint,
    pub m_dist: c_uint,
    pub m_counter: c_uint,
    pub m_num_extra: c_uint,
    pub m_table_sizes: [c_uint; TINFL_MAX_HUFF_TABLES],
    pub m_bit_buf: c_uint,
    pub m_dist_from_out_buf_start: usize,
    pub m_tables: [tinfl_huff_table; TINFL_MAX_HUFF_TABLES],
    pub m_raw_header: [u8; 4],
    pub m_len_codes: [u8; TINFL_MAX_HUFF_SYMBOLS_0 + TINFL_MAX_HUFF_SYMBOLS_1 + 137],
}

#[allow(bad_style)]
extern {
    pub fn tinfl_decompress(r: *mut tinfl_decompressor,
                            pIn_buf_next: *const u8,
                            pIn_buf_size: *mut size_t,
                            pOut_buf_start: *mut u8,
                            pOut_buf_next: *mut u8,
                            pOut_buf_size: *mut size_t,
                            decomp_flags: c_uint) -> c_int;
}
