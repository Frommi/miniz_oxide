#![allow(dead_code)]

use ::libc::*;
use std::slice;
use std::mem;
use std::cmp;
use std::io;
use std::io::{Cursor, Seek, SeekFrom, Write};
use std::ptr;

use MZError;
use MZFlush;
mod tdef_oxide;
pub use self::tdef_oxide::*;

pub type PutBufFuncPtrNotNull = unsafe extern "C" fn(*const c_void, c_int, *mut c_void) -> bool;
pub type PutBufFuncPtr = Option<PutBufFuncPtrNotNull>;

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

impl From<MZFlush> for TDEFLFlush {
    fn from(flush: MZFlush) -> Self {
        match flush {
            MZFlush::None => TDEFLFlush::None,
            MZFlush::Sync => TDEFLFlush::Sync,
            MZFlush::Full => TDEFLFlush::Full,
            MZFlush::Finish => TDEFLFlush::Finish,
            _ => TDEFLFlush::None // TODO: ??? What to do ???
        }
    }
}

impl TDEFLFlush {
    pub fn new(flush: c_int) -> Result<Self, MZError> {
        match flush {
            0 => Ok(TDEFLFlush::None),
            2 => Ok(TDEFLFlush::Sync),
            3 => Ok(TDEFLFlush::Full),
            4 => Ok(TDEFLFlush::Finish),
            _ => Err(MZError::Param)
        }
    }
}

pub const TDEFL_LZ_CODE_BUF_SIZE: usize = 64 * 1024;
pub const TDEFL_OUT_BUF_SIZE: usize = (TDEFL_LZ_CODE_BUF_SIZE * 13) / 10;
pub const TDEFL_MAX_HUFF_SYMBOLS: usize = 288;
pub const TDEFL_LZ_HASH_BITS: i32 = 15;
pub const TDEFL_LEVEL1_HASH_SIZE_MASK: u32 = 4095;
pub const TDEFL_LZ_HASH_SHIFT: i32 = (TDEFL_LZ_HASH_BITS + 2) / 3;
pub const TDEFL_LZ_HASH_SIZE: usize = 1 << TDEFL_LZ_HASH_BITS;

pub const TDEFL_MAX_HUFF_TABLES: usize = 3;
pub const TDEFL_MAX_HUFF_SYMBOLS_0: usize = 288;
pub const TDEFL_MAX_HUFF_SYMBOLS_1: usize = 32;
pub const TDEFL_MAX_HUFF_SYMBOLS_2: usize = 19;
pub const TDEFL_LZ_DICT_SIZE: usize = 32768;
pub const TDEFL_LZ_DICT_SIZE_MASK: u32 = TDEFL_LZ_DICT_SIZE as u32 - 1;
pub const TDEFL_MIN_MATCH_LEN: u32 = 3;
pub const TDEFL_MAX_MATCH_LEN: usize = 258;

pub const TDEFL_WRITE_ZLIB_HEADER: u32 = 0x01000;
pub const TDEFL_COMPUTE_ADLER32: u32 = 0x02000;
pub const TDEFL_GREEDY_PARSING_FLAG: u32 = 0x04000;
pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: u32 = 0x08000;
pub const TDEFL_RLE_MATCHES: u32 = 0x10000;
pub const TDEFL_FILTER_MATCHES: u32 = 0x20000;
pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: u32 = 0x40000;
pub const TDEFL_FORCE_ALL_RAW_BLOCKS: u32 = 0x80000;

pub const TDEFL_DEFAULT_MAX_PROBES: i32 = 128;
pub const TDEFL_MAX_PROBES_MASK: i32 = 0xFFF;

const TDEFL_MAX_SUPPORTED_HUFF_CODESIZE: usize = 32;

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

const TDEFL_NUM_PROBES: [u32; 11] = [0, 1, 6, 32, 16, 32, 128, 256, 512, 768, 1500];

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SymFreq {
    key: u16,
    sym_index: u16,
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress(
    d: Option<&mut CompressorOxide>,
    in_buf: *const c_void,
    in_size: Option<&mut usize>,
    out_buf: *mut c_void,
    out_size: Option<&mut usize>,
    flush: TDEFLFlush,
) -> TDEFLStatus {
    let res = match d {
        None => {
            in_size.map(|size| *size = 0);
            out_size.map(|size| *size = 0);
            (TDEFLStatus::BadParam, 0, 0)
        },
        Some(compressor) => {
            let callback_res = CallbackOxide::new(
                compressor.callback_func.clone(),
                in_buf,
                in_size,
                out_buf,
                out_size,
            );

            if let Ok(mut callback) = callback_res {
                let res = compress(compressor, &mut callback, flush);
                callback.update_size(Some(res.1), Some(res.2));
                res
            } else {
                (TDEFLStatus::BadParam, 0, 0)
            }
        }
    };
    res.0
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_buffer(
    d: Option<&mut CompressorOxide>,
    in_buf: *const c_void,
    mut in_size: usize,
    flush: TDEFLFlush,
) -> TDEFLStatus {
    tdefl_compress(d, in_buf, Some(&mut in_size), ptr::null_mut(), None, flush)
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_init(
    d: Option<&mut CompressorOxide>,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> TDEFLStatus {
    if let Some(d) = d {
        *d = CompressorOxide::new(
            put_buf_func.map(|func|
                CallbackFunc { put_buf_func: func, put_buf_user: put_buf_user }
            ),
            flags as u32,
        );
        TDEFLStatus::Okay
    } else {
        TDEFLStatus::BadParam
    }
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_get_prev_return_status(
    d: Option<&mut CompressorOxide>
) -> TDEFLStatus {
    d.map_or(TDEFLStatus::Okay, |d| d.params.prev_return_status)
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_get_adler32(d: Option<&mut CompressorOxide>) -> c_uint {
    d.map_or(::MZ_ADLER32_INIT as u32, |d| d.params.adler32)
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_mem_to_output(
    buf: *const c_void,
    buf_len: usize,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> bool {
    if let Some(put_buf_func) = put_buf_func {
        let compressor = ::miniz_def_alloc_func(
            ptr::null_mut(),
            1,
            mem::size_of::<CompressorOxide>()
        ) as *mut CompressorOxide;

        *compressor = CompressorOxide::new(Some(CallbackFunc {
            put_buf_func: put_buf_func,
            put_buf_user: put_buf_user
        }), flags as u32);

        let res = tdefl_compress_buffer(compressor.as_mut(), buf, buf_len, TDEFLFlush::Finish) == TDEFLStatus::Done;
        ::miniz_def_free_func(ptr::null_mut(), compressor as *mut c_void);
        res
    } else {
        false
    }
}

struct BufferUser {
    pub size: usize,
    pub capacity: usize,
    pub buf: *mut u8,
    pub expandable: bool,
}

pub unsafe extern "C" fn output_buffer_putter(buf: *const c_void, len: c_int, user: *mut c_void) -> bool {
    let user = (user as *mut BufferUser).as_mut();
    match user {
        None => false,
        Some(user) => {
            let new_size = user.size + len as usize;
            if new_size > user.capacity {
                if !user.expandable { return false }
                let mut new_capacity = cmp::max(user.capacity, 128);
                while new_size > new_capacity {
                    new_capacity <<= 1;
                }

                let new_buf = ::miniz_def_realloc_func(
                    ptr::null_mut(),
                    user.buf as *mut c_void,
                    1,
                    new_capacity
                );

                if new_buf.is_null() {
                    return false;
                }

                user.buf = new_buf as *mut u8;
                user.capacity = new_capacity;
            }

            ptr::copy_nonoverlapping(
                buf as *const u8,
                user.buf.offset(user.size as isize),
                len as usize
            );
            user.size = new_size;
            true
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_mem_to_heap(
    src_buf: *const c_void,
    src_buf_len: usize,
    out_len: *mut usize,
    flags: c_int,
) -> *mut c_void {
    match out_len.as_mut() {
        None => return ptr::null_mut(),
        Some(len) => {
            *len = 0;

            let mut buffer_user = BufferUser {
                size: 0,
                capacity: 0,
                buf: ptr::null_mut(),
                expandable: true,
            };

            if !tdefl_compress_mem_to_output(
                src_buf,
                src_buf_len,
                Some(output_buffer_putter),
                &mut buffer_user as *mut BufferUser as *mut c_void,
                flags
            ) {
                ptr::null_mut()
            } else {
                *len = buffer_user.size;
                buffer_user.buf as *mut c_void
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_mem_to_mem(
    out_buf: *mut c_void,
    out_buf_len: usize,
    src_buf: *const c_void,
    src_buf_len: usize,
    flags: c_int,
) -> usize {
    if out_buf.is_null() { return 0 }
    let mut buffer_user = BufferUser {
        size: 0,
        capacity: out_buf_len,
        buf: out_buf as *mut u8,
        expandable: false,
    };

    if tdefl_compress_mem_to_output(
        src_buf,
        src_buf_len,
        Some(output_buffer_putter),
        &mut buffer_user as *mut BufferUser as *mut c_void,
        flags
    ) {
        buffer_user.size
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn tdefl_create_comp_flags_from_zip_params(
    level: c_int,
    window_bits: c_int,
    strategy: c_int,
) -> c_uint {
    create_comp_flags_from_zip_params(level, window_bits, strategy)
}
