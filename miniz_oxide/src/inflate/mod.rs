use ::libc::*;
use std::{mem, usize};
use std::io::Cursor;

mod tinfl_oxide;
mod output_buffer;
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

    /// Look for a huffman code in the fast lookup table.
    /// The code is stored in the lower 9 bits, the length in the next 6.
    /// If the returned value is negative, the code wasn't found in the
    /// fast lookup table and the tree has to be traversed to find the code.
    #[inline]
    fn fast_lookup(&self, bit_buf: BitBuffer) -> i16 {
        self.look_up[(bit_buf & (TINFL_FAST_LOOKUP_SIZE - 1) as BitBuffer) as usize]
    }

    /// Get the huffman code and the length from the huffman tree.
    #[inline]
    fn tree_lookup(
        &self,
        fast_symbol: i32,
        bit_buf: BitBuffer,
        mut code_len: u32,
    ) -> (i32, u32) {
        let mut symbol = fast_symbol;
        loop {
            symbol = self.tree[(!symbol + ((bit_buf >> code_len) & 1) as i32) as usize] as i32;
            code_len += 1;
            if symbol >= 0 {
                break;
            }
        }
        (symbol, code_len)
    }

    #[inline]
    /// Look up a huffman code from the bits in the provided bit buffer.
    fn lookup(&self, bit_buf: BitBuffer) -> (i32, u32) {
        let symbol = self.fast_lookup(bit_buf).into();
        if symbol >= 0 {
            (symbol, (symbol >> 9) as u32)
        } else {
            // We didn't get a code from the fast lookup table, so check the tree instead.
            self.tree_lookup(symbol.into(), bit_buf, TINFL_FAST_LOOKUP_BITS.into())
        }
    }
}

const TINFL_MAX_HUFF_TABLES: usize = 3;
const TINFL_MAX_HUFF_SYMBOLS_0: usize = 288;
const TINFL_MAX_HUFF_SYMBOLS_1: usize = 32;
const TINFL_MAX_HUFF_SYMBOLS_2: usize = 19;
const TINFL_FAST_LOOKUP_BITS: u8 = 10;
const TINFL_FAST_LOOKUP_SIZE: u32 = 1 << TINFL_FAST_LOOKUP_BITS;
const LITLEN_TABLE: usize = 0;
const DIST_TABLE: usize = 1;
const HUFFLEN_TABLE: usize = 2;


pub mod inflate_flags {
    pub const TINFL_FLAG_PARSE_ZLIB_HEADER: u32 = 1;
    pub const TINFL_FLAG_HAS_MORE_INPUT: u32 = 2;
    pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: u32 = 4;
    pub const TINFL_FLAG_COMPUTE_ADLER32: u32 = 8;
}

use self::inflate_flags::*;

pub const TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS: i32 = -4;
pub const TINFL_STATUS_BAD_PARAM: i32 = -3;
pub const TINFL_STATUS_ADLER32_MISMATCH: i32 = -2;
pub const TINFL_STATUS_FAILED: i32 = -1;
pub const TINFL_STATUS_DONE: i32 = 0;
pub const TINFL_STATUS_NEEDS_MORE_INPUT: i32 = 1;
pub const TINFL_STATUS_HAS_MORE_OUTPUT: i32 = 2;

#[repr(i8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TINFLStatus {
    FailedCannotMakeProgress = TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS as i8,
    BadParam = TINFL_STATUS_BAD_PARAM as i8,
    Adler32Mismatch = TINFL_STATUS_ADLER32_MISMATCH as i8,
    Failed = TINFL_STATUS_FAILED as i8,
    Done = TINFL_STATUS_DONE as i8,
    NeedsMoreInput = TINFL_STATUS_NEEDS_MORE_INPUT as i8,
    HasMoreOutput = TINFL_STATUS_HAS_MORE_OUTPUT as i8,
}

impl TINFLStatus {
    pub fn from_i32(value: i32) -> Option<TINFLStatus> {
        use self::TINFLStatus::*;
        match value {
            TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS => Some(FailedCannotMakeProgress),
            TINFL_STATUS_BAD_PARAM => Some(BadParam),
            TINFL_STATUS_ADLER32_MISMATCH => Some(Adler32Mismatch),
            TINFL_STATUS_FAILED => Some(Failed),
            TINFL_STATUS_DONE => Some(Done),
            TINFL_STATUS_NEEDS_MORE_INPUT => Some(NeedsMoreInput),
            TINFL_STATUS_HAS_MORE_OUTPUT => Some(HasMoreOutput),
            _ => None,
        }
    }
}

pub const TDEFL_WRITE_ZLIB_HEADER: u32 = 0x01000;
pub const TDEFL_COMPUTE_ADLER32: u32 = 0x02000;
pub const TDEFL_GREEDY_PARSING_FLAG: u32 = 0x04000;
pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: u32 = 0x08000;
pub const TDEFL_RLE_MATCHES: u32 = 0x10000;
pub const TDEFL_FILTER_MATCHES: u32 = 0x20000;
pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: u32 = 0x40000;
pub const TDEFL_FORCE_ALL_RAW_BLOCKS: u32 = 0x80000;

const MIN_TABLE_SIZES: [u16; 3] = [257, 1, 4];
const LENGTH_DEZIGZAG: [u8; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

#[cfg(target_pointer_width = "64")]
type BitBuffer = u64;

#[cfg(not(target_pointer_width = "64"))]
type BitBuffer = u32;

#[repr(C)]
#[allow(bad_style)]
pub struct tinfl_decompressor {
    //pub state: u32,
    pub state: tinfl_oxide::State,
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
            state: tinfl_oxide::State::Start,
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
        decomp.state = tinfl_oxide::State::Start;
        decomp
    }
}

pub const TINFL_DECOMPRESS_MEM_TO_MEM_FAILED: size_t = usize::MAX;

/// Decompress the deflate-encoded data in `input` to a vector.
///
/// Returns a status and an integer representing where the decompressor failed on failure.
#[inline]
pub fn decompress_to_vec(input: &[u8]) -> Result<Vec<u8>, (TINFLStatus, u32)> {
    decompress_to_vec_inner(input, 0)
}

/// Decompress the deflate-encoded data (with a zlib wrapper) in `input` to a vector.
///
/// Returns a status and an integer representing where the decompressor failed on failure.
#[inline]
pub fn decompress_to_vec_zlib(input: &[u8]) -> Result<Vec<u8>, (TINFLStatus, u32)> {
    decompress_to_vec_inner(input, TINFL_FLAG_PARSE_ZLIB_HEADER)
}

#[inline]
fn decompress_to_vec_inner(input: &[u8], flags: u32) -> Result<Vec<u8>,(TINFLStatus, u32)> {
    let flags = flags | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let mut ret = Vec::with_capacity(input.len() * 2);

    // # Unsafe
    // We trust decompress_oxide to not read the unitialized bytes as it's wrapped
    // in a cursor that's position is set to the end of the initialized data.
    unsafe {
        let cap = ret.capacity();
        ret.set_len(cap);
    };
    let mut decomp = unsafe {
        tinfl_decompressor::with_init_state_only()
    };

    let mut in_pos = 0;
    let mut out_pos = 0;
    loop {
        let (status, in_consumed, out_consumed) = {
            // Wrap the whole output slice so we know we have enough of the
            // decompressed data for matches.
            let mut c = Cursor::new(ret.as_mut_slice());
            c.set_position(out_pos as u64);
            decompress_oxide(
                &mut decomp,
                &input[in_pos..],
                &mut c,
                flags)
        };
        in_pos += in_consumed;
        out_pos += out_consumed;

        match status {
            TINFLStatus::Done => {
                ret.truncate(out_pos);
                return Ok(ret);
            },
            TINFLStatus::HasMoreOutput => {
                // We need more space so extend the buffer.
                ret.reserve(out_pos);
                // # Unsafe
                // We trust decompress_oxide to not read the unitialized bytes as it's wrapped
                // in a cursor that's position is set to the end of the initialized data.
                unsafe {
                    let cap = ret.capacity();
                    ret.set_len(cap);
                }
            },
            // TODO: Return enum directly.
            _ => return Err((status, decomp.state as u32))
        }
    }
}


#[cfg(test)]
mod test {
    use super::decompress_to_vec_zlib;

    #[test]
    fn decompress_vec() {
        let encoded =
            [120, 156, 243, 72, 205, 201, 201, 215, 81,
             168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19];
        let res = decompress_to_vec_zlib(&encoded[..]).unwrap();
        assert_eq!(res.as_slice(), &b"Hello, zlib!"[..]);
    }
}
