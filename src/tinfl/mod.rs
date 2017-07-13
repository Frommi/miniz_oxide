extern crate libc;

use self::libc::*;

pub const TINFL_LZ_DICT_SIZE: usize = 32768;

pub const TINFL_STATUS_NEEDS_MORE_INPUT: c_int = 1;

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
