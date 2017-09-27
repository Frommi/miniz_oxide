//! This module contains functionality for decompression.
//! `decompress_oxide` is the main decompression function.

use std::{mem, usize};
use std::io::Cursor;

mod tinfl_oxide;
mod output_buffer;
pub use self::tinfl_oxide::*;

pub const TINFL_LZ_DICT_SIZE: usize = 32_768;

/// A struct containing huffman code lengths and the huffman code tree used by the decompressor.
#[repr(C)]
struct HuffmanTable {
    /// Length of the code at each index.
    pub code_size: [u8; 288],
    /// Fast lookup table for shorter huffman codes.
    ///
    /// See `HuffmanTable::fast_lookup`.
    pub look_up: [i16; 1024],
    /// Full huffman tree.
    ///
    /// Positive values are edge nodes/symbols, negative values are
    /// parent nodes/references to other nodes.
    pub tree: [i16; 576],
}

impl HuffmanTable {
    fn new() -> HuffmanTable {
        HuffmanTable {
            code_size: [0; 288],
            look_up: [0; 1024],
            tree: [0; 576],
        }
    }

    /// Look for a symbol in the fast lookup table.
    /// The symbol is stored in the lower 9 bits, the length in the next 6.
    /// If the returned value is negative, the code wasn't found in the
    /// fast lookup table and the full tree has to be traversed to find the code.
    #[inline]
    fn fast_lookup(&self, bit_buf: BitBuffer) -> i16 {
        self.look_up[(bit_buf & (TINFL_FAST_LOOKUP_SIZE - 1) as BitBuffer) as usize]
    }

    /// Get the symbol and the code length from the huffman tree.
    #[inline]
    fn tree_lookup(&self, fast_symbol: i32, bit_buf: BitBuffer, mut code_len: u32) -> (i32, u32) {
        let mut symbol = fast_symbol;
        // We step through the tree until we encounter a positive value, which indicates a
        // symbol.
        loop {
            // symbol here indicates the position of the left (0) node, if the next bit is 1
            // we add 1 to the lookup position to get the right node.
            symbol = self.tree[(!symbol + ((bit_buf >> code_len) & 1) as i32) as usize] as i32;
            code_len += 1;
            if symbol >= 0 {
                break;
            }
        }
        (symbol, code_len)
    }

    #[inline]
    /// Look up a symbol and code length from the bits in the provided bit buffer.
    fn lookup(&self, bit_buf: BitBuffer) -> (i32, u32) {
        let symbol = self.fast_lookup(bit_buf).into();
        if symbol >= 0 {
            (symbol, (symbol >> 9) as u32)
        } else {
            // We didn't get a symbol from the fast lookup table, so check the tree instead.
            self.tree_lookup(symbol.into(), bit_buf, TINFL_FAST_LOOKUP_BITS.into())
        }
    }
}

/// The number of huffman tables used.
const TINFL_MAX_HUFF_TABLES: usize = 3;
/// The length of the first (literal/length) huffman table.
const TINFL_MAX_HUFF_SYMBOLS_0: usize = 288;
/// The length of the second (distance) huffman table.
const TINFL_MAX_HUFF_SYMBOLS_1: usize = 32;
/// The length of the last (huffman code length) huffman table.
const _TINFL_MAX_HUFF_SYMBOLS_2: usize = 19;
/// The maximum length of a code that can be looked up in the fast lookup table.
const TINFL_FAST_LOOKUP_BITS: u8 = 10;
/// The size of the fast lookup table.
const TINFL_FAST_LOOKUP_SIZE: u32 = 1 << TINFL_FAST_LOOKUP_BITS;
const LITLEN_TABLE: usize = 0;
const DIST_TABLE: usize = 1;
const HUFFLEN_TABLE: usize = 2;


pub mod inflate_flags {
    /// Should we try to parse a zlib header?
    pub const TINFL_FLAG_PARSE_ZLIB_HEADER: u32 = 1;
    /// There is more input that hasn't been given to the decompressor yet.
    pub const TINFL_FLAG_HAS_MORE_INPUT: u32 = 2;
    /// The output buffer should not wrap around.
    pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: u32 = 4;
    /// Should we calculate the adler32 checksum of the output data?
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
    /// More input data was expected, but the caller indicated that there was more data, so the
    /// input stream is likely truncated.
    FailedCannotMakeProgress = TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS as i8,
    /// One or more of the input parameters were invalid.
    BadParam = TINFL_STATUS_BAD_PARAM as i8,
    /// The decompression went fine, but the adler32 checksum did not match the one
    /// provided in the header.
    Adler32Mismatch = TINFL_STATUS_ADLER32_MISMATCH as i8,
    /// Failed to decompress due to invalid data.
    Failed = TINFL_STATUS_FAILED as i8,
    /// Finished decomression without issues.
    Done = TINFL_STATUS_DONE as i8,
    /// The decompressor needs more input data to continue decompressing.
    NeedsMoreInput = TINFL_STATUS_NEEDS_MORE_INPUT as i8,
    /// There is still pending data that didn't fit in the output buffer.
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

const MIN_TABLE_SIZES: [u16; 3] = [257, 1, 4];

#[cfg(target_pointer_width = "64")]
type BitBuffer = u64;

#[cfg(not(target_pointer_width = "64"))]
type BitBuffer = u32;

/// Main decompression struct.
///
/// This is repr(C) to be usable in the C API.
#[repr(C)]
#[allow(bad_style)]
pub struct tinfl_decompressor {
    /// Current state of the decompressor.
    state: tinfl_oxide::State,
    /// Number of bits in the bit buffer.
    num_bits: u32,
    /// Zlib CMF
    z_header0: u32,
    /// Zlib FLG
    z_header1: u32,
    /// Adler32 checksum from the zlib header.
    z_adler32: u32,
    /// 1 if the current block is the last block, 0 otherwise.
    finish: u32,
    /// The type of the current block.
    block_type: u32,
    /// 1 if the adler32 value should be checked.
    check_adler32: u32,
    /// Last match distance.
    dist: u32,
    /// Variable used for match length, symbols, and a number of other things.
    counter: u32,
    /// Number of extra bits for the last length or distance code.
    num_extra: u32,
    /// Number of entries in each huffman table.
    table_sizes: [u32; TINFL_MAX_HUFF_TABLES],
    /// Buffer of input data.
    bit_buf: BitBuffer,
    /// Position in the output buffer.
    dist_from_out_buf_start: usize,
    /// Huffman tables.
    tables: [HuffmanTable; TINFL_MAX_HUFF_TABLES],
    /// Raw block header.
    raw_header: [u8; 4],
    /// Huffman length codes.
    len_codes: [u8; TINFL_MAX_HUFF_SYMBOLS_0 + TINFL_MAX_HUFF_SYMBOLS_1 + 137],
}

impl tinfl_decompressor {
    /// Create a new tinfl_decompressor with all fields set to 0.
    pub fn new() -> tinfl_decompressor {
        tinfl_decompressor::default()
    }

    /// Create a new tinfl_decompressor with all fields set to 0.
    pub fn default() -> tinfl_decompressor {
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
            tables: [HuffmanTable::new(), HuffmanTable::new(), HuffmanTable::new()],
            raw_header: [0; 4],
            len_codes: [0; TINFL_MAX_HUFF_SYMBOLS_0 + TINFL_MAX_HUFF_SYMBOLS_1 + 137],
        }
    }

    /// Set the current state to `Start`.
    #[inline]
    pub fn init(&mut self) {
        self.state = tinfl_oxide::State::Start;
    }

    /// Create a new decompressor with only the state field initialized.
    ///
    /// This is how it's created in miniz. Unsafe due to uninitialized values.
    #[inline]
    pub unsafe fn with_init_state_only() -> tinfl_decompressor {
        let mut decomp: tinfl_decompressor = mem::uninitialized();
        decomp.state = tinfl_oxide::State::Start;
        decomp
    }

    /// Returns the adler32 checksum of the currently decompressed data.
    #[inline]
    pub fn adler32(&self) -> Option<u32> {
        if self.state != tinfl_oxide::State::Start &&
            self.state != tinfl_oxide::State::BadZlibHeader && self.z_header0 != 0
        {
            Some(self.check_adler32)
        } else {
            None
        }
    }
}

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
    decompress_to_vec_inner(input, inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER)
}

fn decompress_to_vec_inner(input: &[u8], flags: u32) -> Result<Vec<u8>, (TINFLStatus, u32)> {
    let flags = flags | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let mut ret = Vec::with_capacity(input.len() * 2);

    // # Unsafe
    // We trust decompress_oxide to not read the unitialized bytes as it's wrapped
    // in a cursor that's position is set to the end of the initialized data.
    unsafe {
        let cap = ret.capacity();
        ret.set_len(cap);
    };
    let mut decomp = unsafe { tinfl_decompressor::with_init_state_only() };

    let mut in_pos = 0;
    let mut out_pos = 0;
    loop {
        let (status, in_consumed, out_consumed) = {
            // Wrap the whole output slice so we know we have enough of the
            // decompressed data for matches.
            let mut c = Cursor::new(ret.as_mut_slice());
            c.set_position(out_pos as u64);
            decompress_oxide(&mut decomp, &input[in_pos..], &mut c, flags)
        };
        in_pos += in_consumed;
        out_pos += out_consumed;

        match status {
            TINFLStatus::Done => {
                ret.truncate(out_pos);
                return Ok(ret);
            }
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
            }
            // TODO: Return enum directly.
            _ => return Err((status, decomp.state as u32)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::decompress_to_vec_zlib;

    #[test]
    fn decompress_vec() {
        let encoded = [
            120,
            156,
            243,
            72,
            205,
            201,
            201,
            215,
            81,
            168,
            202,
            201,
            76,
            82,
            4,
            0,
            27,
            101,
            4,
            19,
        ];
        let res = decompress_to_vec_zlib(&encoded[..]).unwrap();
        assert_eq!(res.as_slice(), &b"Hello, zlib!"[..]);
    }
}
