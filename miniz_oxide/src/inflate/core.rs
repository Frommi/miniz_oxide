//! Streaming decompression functionality.

use super::*;
use crate::shared::{update_adler32, HUFFMAN_LENGTH_ORDER};

use ::core::convert::TryInto;
use ::core::{cmp, slice};

use self::output_buffer::OutputBuffer;

pub const TINFL_LZ_DICT_SIZE: usize = 32_768;

/// A struct containing huffman code lengths and the huffman code tree used by the decompressor.
struct HuffmanTable {
    /// Length of the code at each index.
    pub code_size: [u8; MAX_HUFF_SYMBOLS_0],
    /// Fast lookup table for shorter huffman codes.
    ///
    /// See `HuffmanTable::fast_lookup`.
    pub look_up: [i16; FAST_LOOKUP_SIZE as usize],
    /// Full huffman tree.
    ///
    /// Positive values are edge nodes/symbols, negative values are
    /// parent nodes/references to other nodes.
    pub tree: [i16; MAX_HUFF_TREE_SIZE],
}

impl HuffmanTable {
    const fn new() -> HuffmanTable {
        HuffmanTable {
            code_size: [0; MAX_HUFF_SYMBOLS_0],
            look_up: [0; FAST_LOOKUP_SIZE as usize],
            tree: [0; MAX_HUFF_TREE_SIZE],
        }
    }

    /// Look for a symbol in the fast lookup table.
    /// The symbol is stored in the lower 9 bits, the length in the next 6.
    /// If the returned value is negative, the code wasn't found in the
    /// fast lookup table and the full tree has to be traversed to find the code.
    #[inline]
    fn fast_lookup(&self, bit_buf: BitBuffer) -> i16 {
        self.look_up[(bit_buf & BitBuffer::from(FAST_LOOKUP_SIZE - 1)) as usize]
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
            symbol = i32::from(self.tree[(!symbol + ((bit_buf >> code_len) & 1) as i32) as usize]);
            code_len += 1;
            if symbol >= 0 {
                break;
            }
        }
        (symbol, code_len)
    }

    #[inline]
    /// Look up a symbol and code length from the bits in the provided bit buffer.
    ///
    /// Returns Some(symbol, length) on success,
    /// None if the length is 0.
    ///
    /// It's possible we could avoid checking for 0 if we can guarantee a sane table.
    /// TODO: Check if a smaller type for code_len helps performance.
    fn lookup(&self, bit_buf: BitBuffer) -> Option<(i32, u32)> {
        let symbol = self.fast_lookup(bit_buf).into();
        if symbol >= 0 {
            if (symbol >> 9) as u32 != 0 {
                Some((symbol, (symbol >> 9) as u32))
            } else {
                // Zero-length code.
                None
            }
        } else {
            // We didn't get a symbol from the fast lookup table, so check the tree instead.
            Some(self.tree_lookup(symbol, bit_buf, FAST_LOOKUP_BITS.into()))
        }
    }
}

/// The number of huffman tables used.
const MAX_HUFF_TABLES: usize = 3;
/// The length of the first (literal/length) huffman table.
const MAX_HUFF_SYMBOLS_0: usize = 288;
/// The length of the second (distance) huffman table.
const MAX_HUFF_SYMBOLS_1: usize = 32;
/// The length of the last (huffman code length) huffman table.
const _MAX_HUFF_SYMBOLS_2: usize = 19;
/// The maximum length of a code that can be looked up in the fast lookup table.
const FAST_LOOKUP_BITS: u8 = 10;
/// The size of the fast lookup table.
const FAST_LOOKUP_SIZE: u32 = 1 << FAST_LOOKUP_BITS;
const MAX_HUFF_TREE_SIZE: usize = MAX_HUFF_SYMBOLS_0 * 2;
const LITLEN_TABLE: usize = 0;
const DIST_TABLE: usize = 1;
const HUFFLEN_TABLE: usize = 2;

/// Flags to [`decompress()`] to control how inflation works.
///
/// These define bits for a bitmask argument.
pub mod inflate_flags {
    /// Should we try to parse a zlib header?
    ///
    /// If unset, the function will expect an RFC1951 deflate stream.  If set, it will expect a
    /// RFC1950 zlib wrapper around the deflate stream.
    pub const TINFL_FLAG_PARSE_ZLIB_HEADER: u32 = 1;

    /// There will be more input that hasn't been given to the decompressor yet.
    ///
    /// This is useful when you want to decompress what you have so far,
    /// even if you know there is probably more input that hasn't gotten here yet (_e.g._, over a
    /// network connection).  When [`decompress()`][super::decompress] reaches the end of the input
    /// without finding the end of the compressed stream, it will return
    /// [`TINFLStatus::NeedsMoreInput`][super::TINFLStatus::NeedsMoreInput] if this is set,
    /// indicating that you should get more data before calling again.  If not set, it will return
    /// [`TINFLStatus::FailedCannotMakeProgress`][super::TINFLStatus::FailedCannotMakeProgress]
    /// suggesting the stream is corrupt, since you claimed it was all there.
    pub const TINFL_FLAG_HAS_MORE_INPUT: u32 = 2;

    /// The output buffer should not wrap around.
    pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: u32 = 4;

    /// Calculate the adler32 checksum of the output data even if we're not inflating a zlib stream.
    ///
    /// If [`TINFL_FLAG_IGNORE_ADLER32`] is specified, it will override this.
    ///
    /// NOTE: Enabling/disabling this between calls to decompress will result in an incorrect
    /// checksum.
    pub const TINFL_FLAG_COMPUTE_ADLER32: u32 = 8;

    /// Ignore adler32 checksum even if we are inflating a zlib stream.
    ///
    /// Overrides [`TINFL_FLAG_COMPUTE_ADLER32`] if both are enabled.
    ///
    /// NOTE: This flag does not exist in miniz as it does not support this and is a
    /// custom addition for miniz_oxide.
    ///
    /// NOTE: Should not be changed from enabled to disabled after decompression has started,
    /// this will result in checksum failure (outside the unlikely event where the checksum happens
    /// to match anyway).
    pub const TINFL_FLAG_IGNORE_ADLER32: u32 = 64;
}

use self::inflate_flags::*;

const MIN_TABLE_SIZES: [u16; 3] = [257, 1, 4];

#[cfg(target_pointer_width = "64")]
type BitBuffer = u64;

#[cfg(not(target_pointer_width = "64"))]
type BitBuffer = u32;

/// Main decompression struct.
///
pub struct DecompressorOxide {
    /// Current state of the decompressor.
    state: core::State,
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
    table_sizes: [u32; MAX_HUFF_TABLES],
    /// Buffer of input data.
    bit_buf: BitBuffer,
    /// Huffman tables.
    tables: [HuffmanTable; MAX_HUFF_TABLES],
    /// Raw block header.
    raw_header: [u8; 4],
    /// Huffman length codes.
    len_codes: [u8; MAX_HUFF_SYMBOLS_0 + MAX_HUFF_SYMBOLS_1 + 137],
}

impl DecompressorOxide {
    /// Create a new tinfl_decompressor with all fields set to 0.
    pub fn new() -> DecompressorOxide {
        DecompressorOxide::default()
    }

    /// Set the current state to `Start`.
    #[inline]
    pub fn init(&mut self) {
        // The rest of the data is reset or overwritten when used.
        self.state = core::State::Start;
    }

    /// Returns the adler32 checksum of the currently decompressed data.
    /// Note: Will return Some(1) if decompressing zlib but ignoring adler32.
    #[inline]
    pub fn adler32(&self) -> Option<u32> {
        if self.state != State::Start && !self.state.is_failure() && self.z_header0 != 0 {
            Some(self.check_adler32)
        } else {
            None
        }
    }

    /// Returns the adler32 that was read from the zlib header if it exists.
    #[inline]
    pub fn adler32_header(&self) -> Option<u32> {
        if self.state != State::Start && self.state != State::BadZlibHeader && self.z_header0 != 0 {
            Some(self.z_adler32)
        } else {
            None
        }
    }
}

impl Default for DecompressorOxide {
    /// Create a new tinfl_decompressor with all fields set to 0.
    #[inline(always)]
    fn default() -> Self {
        DecompressorOxide {
            state: core::State::Start,
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
            table_sizes: [0; MAX_HUFF_TABLES],
            bit_buf: 0,
            // TODO:(oyvindln) Check that copies here are optimized out in release mode.
            tables: [
                HuffmanTable::new(),
                HuffmanTable::new(),
                HuffmanTable::new(),
            ],
            raw_header: [0; 4],
            len_codes: [0; MAX_HUFF_SYMBOLS_0 + MAX_HUFF_SYMBOLS_1 + 137],
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
enum State {
    Start = 0,
    ReadZlibCmf,
    ReadZlibFlg,
    ReadBlockHeader,
    BlockTypeNoCompression,
    RawHeader,
    RawMemcpy1,
    RawMemcpy2,
    ReadTableSizes,
    ReadHufflenTableCodeSize,
    ReadLitlenDistTablesCodeSize,
    ReadExtraBitsCodeSize,
    DecodeLitlen,
    WriteSymbol,
    ReadExtraBitsLitlen,
    DecodeDistance,
    ReadExtraBitsDistance,
    RawReadFirstByte,
    RawStoreFirstByte,
    WriteLenBytesToEnd,
    BlockDone,
    HuffDecodeOuterLoop1,
    HuffDecodeOuterLoop2,
    ReadAdler32,

    DoneForever,

    // Failure states.
    BlockTypeUnexpected,
    BadCodeSizeSum,
    BadDistOrLiteralTableLength,
    BadTotalSymbols,
    BadZlibHeader,
    DistanceOutOfBounds,
    BadRawLength,
    BadCodeSizeDistPrevLookup,
    InvalidLitlen,
    InvalidDist,
    InvalidCodeLen,
}

impl State {
    fn is_failure(self) -> bool {
        match self {
            BlockTypeUnexpected => true,
            BadCodeSizeSum => true,
            BadDistOrLiteralTableLength => true,
            BadTotalSymbols => true,
            BadZlibHeader => true,
            DistanceOutOfBounds => true,
            BadRawLength => true,
            BadCodeSizeDistPrevLookup => true,
            InvalidLitlen => true,
            InvalidDist => true,
            _ => false,
        }
    }

    #[inline]
    fn begin(&mut self, new_state: State) {
        *self = new_state;
    }
}

use self::State::*;

// Not sure why miniz uses 32-bit values for these, maybe alignment/cache again?
// # Optimization
// We add a extra value at the end and make the tables 32 elements long
// so we can use a mask to avoid bounds checks.
// The invalid values are set to something high enough to avoid underflowing
// the match length.
/// Base length for each length code.
///
/// The base is used together with the value of the extra bits to decode the actual
/// length/distance values in a match.
#[rustfmt::skip]
const LENGTH_BASE: [u16; 32] = [
    3,  4,  5,  6,  7,  8,  9,  10,  11,  13,  15,  17,  19,  23,  27,  31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 512, 512, 512
];

/// Number of extra bits for each length code.
#[rustfmt::skip]
const LENGTH_EXTRA: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
    3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0, 0, 0, 0
];

/// Base length for each distance code.
#[rustfmt::skip]
const DIST_BASE: [u16; 32] = [
    1,    2,    3,    4,    5,    7,      9,      13,     17,     25,    33,
    49,   65,   97,   129,  193,  257,    385,    513,    769,    1025,  1537,
    2049, 3073, 4097, 6145, 8193, 12_289, 16_385, 24_577, 32_768, 32_768
];

/// Number of extra bits for each distance code.
#[rustfmt::skip]
const DIST_EXTRA: [u8; 32] = [
    0, 0, 0, 0, 1, 1, 2,  2,  3,  3,  4,  4,  5,  5,  6,  6,
    7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 13, 13
];

/// The mask used when indexing the base/extra arrays.
const BASE_EXTRA_MASK: usize = 32 - 1;

/// Sets the value of all the elements of the slice to `val`.
#[inline]
fn memset<T: Copy>(slice: &mut [T], val: T) {
    for x in slice {
        *x = val
    }
}

/// Read an le u16 value from the slice iterator.
///
/// # Panics
/// Panics if there are less than two bytes left.
#[inline]
fn read_u16_le(iter: &mut slice::Iter<u8>) -> u16 {
    let ret = {
        let two_bytes = iter.as_ref()[..2].try_into().unwrap();
        u16::from_le_bytes(two_bytes)
    };
    iter.nth(1);
    ret
}

/// Read an le u32 value from the slice iterator.
///
/// # Panics
/// Panics if there are less than four bytes left.
#[inline(always)]
#[cfg(target_pointer_width = "64")]
fn read_u32_le(iter: &mut slice::Iter<u8>) -> u32 {
    let ret = {
        let four_bytes: [u8; 4] = iter.as_ref()[..4].try_into().unwrap();
        u32::from_le_bytes(four_bytes)
    };
    iter.nth(3);
    ret
}

/// Ensure that there is data in the bit buffer.
///
/// On 64-bit platform, we use a 64-bit value so this will
/// result in there being at least 32 bits in the bit buffer.
/// This function assumes that there is at least 4 bytes left in the input buffer.
#[inline(always)]
#[cfg(target_pointer_width = "64")]
fn fill_bit_buffer(l: &mut LocalVars, in_iter: &mut slice::Iter<u8>) {
    // Read four bytes into the buffer at once.
    if l.num_bits < 30 {
        l.bit_buf |= BitBuffer::from(read_u32_le(in_iter)) << l.num_bits;
        l.num_bits += 32;
    }
}

/// Same as previous, but for non-64-bit platforms.
/// Ensures at least 16 bits are present, requires at least 2 bytes in the in buffer.
#[inline(always)]
#[cfg(not(target_pointer_width = "64"))]
fn fill_bit_buffer(l: &mut LocalVars, in_iter: &mut slice::Iter<u8>) {
    // If the buffer is 32-bit wide, read 2 bytes instead.
    if l.num_bits < 15 {
        l.bit_buf |= BitBuffer::from(read_u16_le(in_iter)) << l.num_bits;
        l.num_bits += 16;
    }
}

/// Check that the zlib header is correct and that there is enough space in the buffer
/// for the window size specified in the header.
///
/// See https://tools.ietf.org/html/rfc1950
#[inline]
fn validate_zlib_header(cmf: u32, flg: u32, flags: u32, mask: usize) -> Action {
    let mut failed =
    // cmf + flg should be divisible by 31.
        (((cmf * 256) + flg) % 31 != 0) ||
    // If this flag is set, a dictionary was used for this zlib compressed data.
    // This is currently not supported by miniz or miniz-oxide
        ((flg & 0b0010_0000) != 0) ||
    // Compression method. Only 8(DEFLATE) is defined by the standard.
        ((cmf & 15) != 8);

    let window_size = 1 << ((cmf >> 4) + 8);
    if (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF) == 0 {
        // Bail if the buffer is wrapping and the window size is larger than the buffer.
        failed |= (mask + 1) < window_size;
    }

    // Zlib doesn't allow window sizes above 32 * 1024.
    failed |= window_size > 32_768;

    if failed {
        Action::Jump(BadZlibHeader)
    } else {
        Action::Jump(ReadBlockHeader)
    }
}

enum Action {
    None,
    Jump(State),
    End(TINFLStatus),
}

/// Try to decode the next huffman code, and puts it in the counter field of the decompressor
/// if successful.
///
/// # Returns
/// The specified action returned from `f` on success,
/// `Action::End` if there are not enough data left to decode a symbol.
fn decode_huffman_code<F>(
    r: &mut DecompressorOxide,
    l: &mut LocalVars,
    table: usize,
    flags: u32,
    in_iter: &mut slice::Iter<u8>,
    f: F,
) -> Action
where
    F: FnOnce(&mut DecompressorOxide, &mut LocalVars, i32) -> Action,
{
    // As the huffman codes can be up to 15 bits long we need at least 15 bits
    // ready in the bit buffer to start decoding the next huffman code.
    if l.num_bits < 15 {
        // First, make sure there is enough data in the bit buffer to decode a huffman code.
        if in_iter.len() < 2 {
            // If there is less than 2 bytes left in the input buffer, we try to look up
            // the huffman code with what's available, and return if that doesn't succeed.
            // Original explanation in miniz:
            // /* TINFL_HUFF_BITBUF_FILL() is only used rarely, when the number of bytes
            //  * remaining in the input buffer falls below 2. */
            // /* It reads just enough bytes from the input stream that are needed to decode
            //  * the next Huffman code (and absolutely no more). It works by trying to fully
            //  * decode a */
            // /* Huffman code by using whatever bits are currently present in the bit buffer.
            //  * If this fails, it reads another byte, and tries again until it succeeds or
            //  * until the */
            // /* bit buffer contains >=15 bits (deflate's max. Huffman code size). */
            loop {
                let mut temp = i32::from(r.tables[table].fast_lookup(l.bit_buf));

                if temp >= 0 {
                    let code_len = (temp >> 9) as u32;
                    if (code_len != 0) && (l.num_bits >= code_len) {
                        break;
                    }
                } else if l.num_bits > FAST_LOOKUP_BITS.into() {
                    let mut code_len = u32::from(FAST_LOOKUP_BITS);
                    loop {
                        temp = i32::from(
                            r.tables[table].tree
                                [(!temp + ((l.bit_buf >> code_len) & 1) as i32) as usize],
                        );
                        code_len += 1;
                        if temp >= 0 || l.num_bits < code_len + 1 {
                            break;
                        }
                    }
                    if temp >= 0 {
                        break;
                    }
                }

                // TODO: miniz jumps straight to here after getting here again after failing to read
                // a byte.
                // Doing that lets miniz avoid re-doing the lookup that that was done in the
                // previous call.
                let mut byte = 0;
                if let a @ Action::End(_) = read_byte(in_iter, flags, |b| {
                    byte = b;
                    Action::None
                }) {
                    return a;
                };

                // Do this outside closure for now to avoid borrowing r.
                l.bit_buf |= BitBuffer::from(byte) << l.num_bits;
                l.num_bits += 8;

                if l.num_bits >= 15 {
                    break;
                }
            }
        } else {
            // There is enough data in the input buffer, so read the next two bytes
            // and add them to the bit buffer.
            // Unwrapping here is fine since we just checked that there are at least two
            // bytes left.
            l.bit_buf |= BitBuffer::from(read_u16_le(in_iter)) << l.num_bits;
            l.num_bits += 16;
        }
    }

    // We now have at least 15 bits in the input buffer.
    let mut symbol = i32::from(r.tables[table].fast_lookup(l.bit_buf));
    let code_len;
    // If the symbol was found in the fast lookup table.
    if symbol >= 0 {
        // Get the length value from the top bits.
        // As we shift down the sign bit, converting to an unsigned value
        // shouldn't overflow.
        code_len = (symbol >> 9) as u32;
        // Mask out the length value.
        symbol &= 511;
    } else {
        let res = r.tables[table].tree_lookup(symbol, l.bit_buf, u32::from(FAST_LOOKUP_BITS));
        symbol = res.0;
        code_len = res.1 as u32;
    };

    if code_len == 0 {
        return Action::Jump(InvalidCodeLen);
    }

    l.bit_buf >>= code_len as u32;
    l.num_bits -= code_len;
    f(r, l, symbol)
}

/// Try to read one byte from `in_iter` and call `f` with the read byte as an argument,
/// returning the result.
/// If reading fails, `Action::End is returned`
#[inline]
fn read_byte<F>(in_iter: &mut slice::Iter<u8>, flags: u32, f: F) -> Action
where
    F: FnOnce(u8) -> Action,
{
    match in_iter.next() {
        None => end_of_input(flags),
        Some(&byte) => f(byte),
    }
}

// TODO: `l: &mut LocalVars` may be slow similar to decompress_fast (even with inline(always))
/// Try to read `amount` number of bits from `in_iter` and call the function `f` with the bits as an
/// an argument after reading, returning the result of that function, or `Action::End` if there are
/// not enough bytes left.
#[inline]
#[allow(clippy::while_immutable_condition)]
fn read_bits<F>(
    l: &mut LocalVars,
    amount: u32,
    in_iter: &mut slice::Iter<u8>,
    flags: u32,
    f: F,
) -> Action
where
    F: FnOnce(&mut LocalVars, BitBuffer) -> Action,
{
    // Clippy gives a false positive warning here due to the closure.
    // Read enough bytes from the input iterator to cover the number of bits we want.
    while l.num_bits < amount {
        match read_byte(in_iter, flags, |byte| {
            l.bit_buf |= BitBuffer::from(byte) << l.num_bits;
            l.num_bits += 8;
            Action::None
        }) {
            Action::None => (),
            // If there are not enough bytes in the input iterator, return and signal that we need
            // more.
            action => return action,
        }
    }

    let bits = l.bit_buf & ((1 << amount) - 1);
    l.bit_buf >>= amount;
    l.num_bits -= amount;
    f(l, bits)
}

#[inline]
fn pad_to_bytes<F>(l: &mut LocalVars, in_iter: &mut slice::Iter<u8>, flags: u32, f: F) -> Action
where
    F: FnOnce(&mut LocalVars) -> Action,
{
    let num_bits = l.num_bits & 7;
    read_bits(l, num_bits, in_iter, flags, |l, _| f(l))
}

#[inline]
fn end_of_input(flags: u32) -> Action {
    Action::End(if flags & TINFL_FLAG_HAS_MORE_INPUT != 0 {
        TINFLStatus::NeedsMoreInput
    } else {
        TINFLStatus::FailedCannotMakeProgress
    })
}

#[inline]
fn undo_bytes(l: &mut LocalVars, max: u32) -> u32 {
    let res = cmp::min(l.num_bits >> 3, max);
    l.num_bits -= res << 3;
    res
}

fn start_static_table(r: &mut DecompressorOxide) {
    r.table_sizes[LITLEN_TABLE] = 288;
    r.table_sizes[DIST_TABLE] = 32;
    memset(&mut r.tables[LITLEN_TABLE].code_size[0..144], 8);
    memset(&mut r.tables[LITLEN_TABLE].code_size[144..256], 9);
    memset(&mut r.tables[LITLEN_TABLE].code_size[256..280], 7);
    memset(&mut r.tables[LITLEN_TABLE].code_size[280..288], 8);
    memset(&mut r.tables[DIST_TABLE].code_size[0..32], 5);
}

/// Equivalent to (0..1024).map(|n| n.reverse_bits())
static REVERSED_BITS_LOOKUP: [u32; 1024] = [
    0, 2147483648, 1073741824, 3221225472, 536870912, 2684354560, 1610612736, 3758096384,
    268435456, 2415919104, 1342177280, 3489660928, 805306368, 2952790016, 1879048192, 4026531840,
    134217728, 2281701376, 1207959552, 3355443200, 671088640, 2818572288, 1744830464, 3892314112,
    402653184, 2550136832, 1476395008, 3623878656, 939524096, 3087007744, 2013265920, 4160749568,
    67108864, 2214592512, 1140850688, 3288334336, 603979776, 2751463424, 1677721600, 3825205248,
    335544320, 2483027968, 1409286144, 3556769792, 872415232, 3019898880, 1946157056, 4093640704,
    201326592, 2348810240, 1275068416, 3422552064, 738197504, 2885681152, 1811939328, 3959422976,
    469762048, 2617245696, 1543503872, 3690987520, 1006632960, 3154116608, 2080374784, 4227858432,
    33554432, 2181038080, 1107296256, 3254779904, 570425344, 2717908992, 1644167168, 3791650816,
    301989888, 2449473536, 1375731712, 3523215360, 838860800, 2986344448, 1912602624, 4060086272,
    167772160, 2315255808, 1241513984, 3388997632, 704643072, 2852126720, 1778384896, 3925868544,
    436207616, 2583691264, 1509949440, 3657433088, 973078528, 3120562176, 2046820352, 4194304000,
    100663296, 2248146944, 1174405120, 3321888768, 637534208, 2785017856, 1711276032, 3858759680,
    369098752, 2516582400, 1442840576, 3590324224, 905969664, 3053453312, 1979711488, 4127195136,
    234881024, 2382364672, 1308622848, 3456106496, 771751936, 2919235584, 1845493760, 3992977408,
    503316480, 2650800128, 1577058304, 3724541952, 1040187392, 3187671040, 2113929216, 4261412864,
    16777216, 2164260864, 1090519040, 3238002688, 553648128, 2701131776, 1627389952, 3774873600,
    285212672, 2432696320, 1358954496, 3506438144, 822083584, 2969567232, 1895825408, 4043309056,
    150994944, 2298478592, 1224736768, 3372220416, 687865856, 2835349504, 1761607680, 3909091328,
    419430400, 2566914048, 1493172224, 3640655872, 956301312, 3103784960, 2030043136, 4177526784,
    83886080, 2231369728, 1157627904, 3305111552, 620756992, 2768240640, 1694498816, 3841982464,
    352321536, 2499805184, 1426063360, 3573547008, 889192448, 3036676096, 1962934272, 4110417920,
    218103808, 2365587456, 1291845632, 3439329280, 754974720, 2902458368, 1828716544, 3976200192,
    486539264, 2634022912, 1560281088, 3707764736, 1023410176, 3170893824, 2097152000, 4244635648,
    50331648, 2197815296, 1124073472, 3271557120, 587202560, 2734686208, 1660944384, 3808428032,
    318767104, 2466250752, 1392508928, 3539992576, 855638016, 3003121664, 1929379840, 4076863488,
    184549376, 2332033024, 1258291200, 3405774848, 721420288, 2868903936, 1795162112, 3942645760,
    452984832, 2600468480, 1526726656, 3674210304, 989855744, 3137339392, 2063597568, 4211081216,
    117440512, 2264924160, 1191182336, 3338665984, 654311424, 2801795072, 1728053248, 3875536896,
    385875968, 2533359616, 1459617792, 3607101440, 922746880, 3070230528, 1996488704, 4143972352,
    251658240, 2399141888, 1325400064, 3472883712, 788529152, 2936012800, 1862270976, 4009754624,
    520093696, 2667577344, 1593835520, 3741319168, 1056964608, 3204448256, 2130706432, 4278190080,
    8388608, 2155872256, 1082130432, 3229614080, 545259520, 2692743168, 1619001344, 3766484992,
    276824064, 2424307712, 1350565888, 3498049536, 813694976, 2961178624, 1887436800, 4034920448,
    142606336, 2290089984, 1216348160, 3363831808, 679477248, 2826960896, 1753219072, 3900702720,
    411041792, 2558525440, 1484783616, 3632267264, 947912704, 3095396352, 2021654528, 4169138176,
    75497472, 2222981120, 1149239296, 3296722944, 612368384, 2759852032, 1686110208, 3833593856,
    343932928, 2491416576, 1417674752, 3565158400, 880803840, 3028287488, 1954545664, 4102029312,
    209715200, 2357198848, 1283457024, 3430940672, 746586112, 2894069760, 1820327936, 3967811584,
    478150656, 2625634304, 1551892480, 3699376128, 1015021568, 3162505216, 2088763392, 4236247040,
    41943040, 2189426688, 1115684864, 3263168512, 578813952, 2726297600, 1652555776, 3800039424,
    310378496, 2457862144, 1384120320, 3531603968, 847249408, 2994733056, 1920991232, 4068474880,
    176160768, 2323644416, 1249902592, 3397386240, 713031680, 2860515328, 1786773504, 3934257152,
    444596224, 2592079872, 1518338048, 3665821696, 981467136, 3128950784, 2055208960, 4202692608,
    109051904, 2256535552, 1182793728, 3330277376, 645922816, 2793406464, 1719664640, 3867148288,
    377487360, 2524971008, 1451229184, 3598712832, 914358272, 3061841920, 1988100096, 4135583744,
    243269632, 2390753280, 1317011456, 3464495104, 780140544, 2927624192, 1853882368, 4001366016,
    511705088, 2659188736, 1585446912, 3732930560, 1048576000, 3196059648, 2122317824, 4269801472,
    25165824, 2172649472, 1098907648, 3246391296, 562036736, 2709520384, 1635778560, 3783262208,
    293601280, 2441084928, 1367343104, 3514826752, 830472192, 2977955840, 1904214016, 4051697664,
    159383552, 2306867200, 1233125376, 3380609024, 696254464, 2843738112, 1769996288, 3917479936,
    427819008, 2575302656, 1501560832, 3649044480, 964689920, 3112173568, 2038431744, 4185915392,
    92274688, 2239758336, 1166016512, 3313500160, 629145600, 2776629248, 1702887424, 3850371072,
    360710144, 2508193792, 1434451968, 3581935616, 897581056, 3045064704, 1971322880, 4118806528,
    226492416, 2373976064, 1300234240, 3447717888, 763363328, 2910846976, 1837105152, 3984588800,
    494927872, 2642411520, 1568669696, 3716153344, 1031798784, 3179282432, 2105540608, 4253024256,
    58720256, 2206203904, 1132462080, 3279945728, 595591168, 2743074816, 1669332992, 3816816640,
    327155712, 2474639360, 1400897536, 3548381184, 864026624, 3011510272, 1937768448, 4085252096,
    192937984, 2340421632, 1266679808, 3414163456, 729808896, 2877292544, 1803550720, 3951034368,
    461373440, 2608857088, 1535115264, 3682598912, 998244352, 3145728000, 2071986176, 4219469824,
    125829120, 2273312768, 1199570944, 3347054592, 662700032, 2810183680, 1736441856, 3883925504,
    394264576, 2541748224, 1468006400, 3615490048, 931135488, 3078619136, 2004877312, 4152360960,
    260046848, 2407530496, 1333788672, 3481272320, 796917760, 2944401408, 1870659584, 4018143232,
    528482304, 2675965952, 1602224128, 3749707776, 1065353216, 3212836864, 2139095040, 4286578688,
    4194304, 2151677952, 1077936128, 3225419776, 541065216, 2688548864, 1614807040, 3762290688,
    272629760, 2420113408, 1346371584, 3493855232, 809500672, 2956984320, 1883242496, 4030726144,
    138412032, 2285895680, 1212153856, 3359637504, 675282944, 2822766592, 1749024768, 3896508416,
    406847488, 2554331136, 1480589312, 3628072960, 943718400, 3091202048, 2017460224, 4164943872,
    71303168, 2218786816, 1145044992, 3292528640, 608174080, 2755657728, 1681915904, 3829399552,
    339738624, 2487222272, 1413480448, 3560964096, 876609536, 3024093184, 1950351360, 4097835008,
    205520896, 2353004544, 1279262720, 3426746368, 742391808, 2889875456, 1816133632, 3963617280,
    473956352, 2621440000, 1547698176, 3695181824, 1010827264, 3158310912, 2084569088, 4232052736,
    37748736, 2185232384, 1111490560, 3258974208, 574619648, 2722103296, 1648361472, 3795845120,
    306184192, 2453667840, 1379926016, 3527409664, 843055104, 2990538752, 1916796928, 4064280576,
    171966464, 2319450112, 1245708288, 3393191936, 708837376, 2856321024, 1782579200, 3930062848,
    440401920, 2587885568, 1514143744, 3661627392, 977272832, 3124756480, 2051014656, 4198498304,
    104857600, 2252341248, 1178599424, 3326083072, 641728512, 2789212160, 1715470336, 3862953984,
    373293056, 2520776704, 1447034880, 3594518528, 910163968, 3057647616, 1983905792, 4131389440,
    239075328, 2386558976, 1312817152, 3460300800, 775946240, 2923429888, 1849688064, 3997171712,
    507510784, 2654994432, 1581252608, 3728736256, 1044381696, 3191865344, 2118123520, 4265607168,
    20971520, 2168455168, 1094713344, 3242196992, 557842432, 2705326080, 1631584256, 3779067904,
    289406976, 2436890624, 1363148800, 3510632448, 826277888, 2973761536, 1900019712, 4047503360,
    155189248, 2302672896, 1228931072, 3376414720, 692060160, 2839543808, 1765801984, 3913285632,
    423624704, 2571108352, 1497366528, 3644850176, 960495616, 3107979264, 2034237440, 4181721088,
    88080384, 2235564032, 1161822208, 3309305856, 624951296, 2772434944, 1698693120, 3846176768,
    356515840, 2503999488, 1430257664, 3577741312, 893386752, 3040870400, 1967128576, 4114612224,
    222298112, 2369781760, 1296039936, 3443523584, 759169024, 2906652672, 1832910848, 3980394496,
    490733568, 2638217216, 1564475392, 3711959040, 1027604480, 3175088128, 2101346304, 4248829952,
    54525952, 2202009600, 1128267776, 3275751424, 591396864, 2738880512, 1665138688, 3812622336,
    322961408, 2470445056, 1396703232, 3544186880, 859832320, 3007315968, 1933574144, 4081057792,
    188743680, 2336227328, 1262485504, 3409969152, 725614592, 2873098240, 1799356416, 3946840064,
    457179136, 2604662784, 1530920960, 3678404608, 994050048, 3141533696, 2067791872, 4215275520,
    121634816, 2269118464, 1195376640, 3342860288, 658505728, 2805989376, 1732247552, 3879731200,
    390070272, 2537553920, 1463812096, 3611295744, 926941184, 3074424832, 2000683008, 4148166656,
    255852544, 2403336192, 1329594368, 3477078016, 792723456, 2940207104, 1866465280, 4013948928,
    524288000, 2671771648, 1598029824, 3745513472, 1061158912, 3208642560, 2134900736, 4282384384,
    12582912, 2160066560, 1086324736, 3233808384, 549453824, 2696937472, 1623195648, 3770679296,
    281018368, 2428502016, 1354760192, 3502243840, 817889280, 2965372928, 1891631104, 4039114752,
    146800640, 2294284288, 1220542464, 3368026112, 683671552, 2831155200, 1757413376, 3904897024,
    415236096, 2562719744, 1488977920, 3636461568, 952107008, 3099590656, 2025848832, 4173332480,
    79691776, 2227175424, 1153433600, 3300917248, 616562688, 2764046336, 1690304512, 3837788160,
    348127232, 2495610880, 1421869056, 3569352704, 884998144, 3032481792, 1958739968, 4106223616,
    213909504, 2361393152, 1287651328, 3435134976, 750780416, 2898264064, 1824522240, 3972005888,
    482344960, 2629828608, 1556086784, 3703570432, 1019215872, 3166699520, 2092957696, 4240441344,
    46137344, 2193620992, 1119879168, 3267362816, 583008256, 2730491904, 1656750080, 3804233728,
    314572800, 2462056448, 1388314624, 3535798272, 851443712, 2998927360, 1925185536, 4072669184,
    180355072, 2327838720, 1254096896, 3401580544, 717225984, 2864709632, 1790967808, 3938451456,
    448790528, 2596274176, 1522532352, 3670016000, 985661440, 3133145088, 2059403264, 4206886912,
    113246208, 2260729856, 1186988032, 3334471680, 650117120, 2797600768, 1723858944, 3871342592,
    381681664, 2529165312, 1455423488, 3602907136, 918552576, 3066036224, 1992294400, 4139778048,
    247463936, 2394947584, 1321205760, 3468689408, 784334848, 2931818496, 1858076672, 4005560320,
    515899392, 2663383040, 1589641216, 3737124864, 1052770304, 3200253952, 2126512128, 4273995776,
    29360128, 2176843776, 1103101952, 3250585600, 566231040, 2713714688, 1639972864, 3787456512,
    297795584, 2445279232, 1371537408, 3519021056, 834666496, 2982150144, 1908408320, 4055891968,
    163577856, 2311061504, 1237319680, 3384803328, 700448768, 2847932416, 1774190592, 3921674240,
    432013312, 2579496960, 1505755136, 3653238784, 968884224, 3116367872, 2042626048, 4190109696,
    96468992, 2243952640, 1170210816, 3317694464, 633339904, 2780823552, 1707081728, 3854565376,
    364904448, 2512388096, 1438646272, 3586129920, 901775360, 3049259008, 1975517184, 4123000832,
    230686720, 2378170368, 1304428544, 3451912192, 767557632, 2915041280, 1841299456, 3988783104,
    499122176, 2646605824, 1572864000, 3720347648, 1035993088, 3183476736, 2109734912, 4257218560,
    62914560, 2210398208, 1136656384, 3284140032, 599785472, 2747269120, 1673527296, 3821010944,
    331350016, 2478833664, 1405091840, 3552575488, 868220928, 3015704576, 1941962752, 4089446400,
    197132288, 2344615936, 1270874112, 3418357760, 734003200, 2881486848, 1807745024, 3955228672,
    465567744, 2613051392, 1539309568, 3686793216, 1002438656, 3149922304, 2076180480, 4223664128,
    130023424, 2277507072, 1203765248, 3351248896, 666894336, 2814377984, 1740636160, 3888119808,
    398458880, 2545942528, 1472200704, 3619684352, 935329792, 3082813440, 2009071616, 4156555264,
    264241152, 2411724800, 1337982976, 3485466624, 801112064, 2948595712, 1874853888, 4022337536,
    532676608, 2680160256, 1606418432, 3753902080, 1069547520, 3217031168, 2143289344, 4290772992,
];

fn init_tree(r: &mut DecompressorOxide, l: &mut LocalVars) -> Action {
    loop {
        let table = &mut r.tables[r.block_type as usize];
        let table_size = r.table_sizes[r.block_type as usize] as usize;
        let mut total_symbols = [0u32; 16];
        let mut next_code = [0u32; 17];
        memset(&mut table.look_up[..], 0);
        memset(&mut table.tree[..], 0);

        for &code_size in &table.code_size[..table_size] {
            total_symbols[code_size as usize] += 1;
        }

        let mut total = 0;
        for i in 1..16 {
            total += total_symbols[i];
            total <<= 1;
            next_code[i + 1] = total;
        }

        // ignore the first element in the check immediately below. doing this
        // rather than slicing off or skipping the first element helps with
        // autovectorization
        total_symbols[0] = 0;

        if total != 65_536 && total_symbols.iter().any(|&n| n > 0) {
            return Action::Jump(BadTotalSymbols);
        }

        let mut tree_next = -1;
        for symbol_index in 0..table_size {
            let mut rev_code = 0;
            let code_size = table.code_size[symbol_index];
            if code_size == 0 {
                continue;
            }

            let mut cur_code = next_code[code_size as usize];
            next_code[code_size as usize] += 1;

            let n = cur_code & (u32::MAX >> (32 - code_size));

            let mut rev_code = if n < 1024 {
                REVERSED_BITS_LOOKUP[n as usize] >> (32 - code_size)
            } else {
                for _ in 0..code_size {
                    rev_code = (rev_code << 1) | (cur_code & 1);
                    cur_code >>= 1;
                }
                rev_code
            };

            if code_size <= FAST_LOOKUP_BITS {
                let k = (i16::from(code_size) << 9) | symbol_index as i16;
                while rev_code < FAST_LOOKUP_SIZE {
                    table.look_up[rev_code as usize] = k;
                    rev_code += 1 << code_size;
                }
                continue;
            }

            let mut tree_cur = table.look_up[(rev_code & (FAST_LOOKUP_SIZE - 1)) as usize];
            if tree_cur == 0 {
                table.look_up[(rev_code & (FAST_LOOKUP_SIZE - 1)) as usize] = tree_next as i16;
                tree_cur = tree_next;
                tree_next -= 2;
            }

            rev_code >>= FAST_LOOKUP_BITS - 1;
            for _ in FAST_LOOKUP_BITS + 1..code_size {
                rev_code >>= 1;
                tree_cur -= (rev_code & 1) as i16;
                if table.tree[(-tree_cur - 1) as usize] == 0 {
                    table.tree[(-tree_cur - 1) as usize] = tree_next as i16;
                    tree_cur = tree_next;
                    tree_next -= 2;
                } else {
                    tree_cur = table.tree[(-tree_cur - 1) as usize];
                }
            }

            rev_code >>= 1;
            tree_cur -= (rev_code & 1) as i16;
            table.tree[(-tree_cur - 1) as usize] = symbol_index as i16;
        }

        if r.block_type == 2 {
            l.counter = 0;
            return Action::Jump(ReadLitlenDistTablesCodeSize);
        }

        if r.block_type == 0 {
            break;
        }
        r.block_type -= 1;
    }

    l.counter = 0;
    Action::Jump(DecodeLitlen)
}

// A helper macro for generating the state machine.
//
// As Rust doesn't have fallthrough on matches, we have to return to the match statement
// and jump for each state change. (Which would ideally be optimized away, but often isn't.)
macro_rules! generate_state {
    ($state: ident, $state_machine: tt, $f: expr) => {
        loop {
            match $f {
                Action::None => continue,
                Action::Jump(new_state) => {
                    $state = new_state;
                    continue $state_machine;
                },
                Action::End(result) => break $state_machine result,
            }
        }
    };
}

#[derive(Copy, Clone)]
struct LocalVars {
    pub bit_buf: BitBuffer,
    pub num_bits: u32,
    pub dist: u32,
    pub counter: u32,
    pub num_extra: u32,
}

#[inline]
fn transfer(
    out_slice: &mut [u8],
    mut source_pos: usize,
    mut out_pos: usize,
    match_len: usize,
    out_buf_size_mask: usize,
) {
    debug_assert!(out_pos > source_pos);
    // special case that comes up surprisingly often. in the case that `source_pos`
    // is 1 less than `out_pos`, we can say that the entire range will be the same
    // value and optimize this to be a simple `memset`
    if out_buf_size_mask == usize::MAX && source_pos.abs_diff(out_pos) == 1 {
        let init = out_slice[out_pos - 1];
        let end = (match_len >> 2) * 4 + out_pos;

        out_slice[out_pos..end].fill(init);
        out_pos = end;
        source_pos = end - 1;
    // if the difference between `source_pos` and `out_pos` is greater than 3, we
    // can do slightly better than the naive case by copying everything at once
    } else if out_buf_size_mask == usize::MAX && source_pos.abs_diff(out_pos) >= 4 {
        for _ in 0..match_len >> 2 {
            out_slice.copy_within(source_pos..=source_pos + 3, out_pos);
            source_pos += 4;
            out_pos += 4;
        }
    } else {
        for _ in 0..match_len >> 2 {
            out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask];
            out_slice[out_pos + 1] = out_slice[(source_pos + 1) & out_buf_size_mask];
            out_slice[out_pos + 2] = out_slice[(source_pos + 2) & out_buf_size_mask];
            out_slice[out_pos + 3] = out_slice[(source_pos + 3) & out_buf_size_mask];
            source_pos += 4;
            out_pos += 4;
        }
    }

    match match_len & 3 {
        0 => (),
        1 => out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask],
        2 => {
            out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask];
            out_slice[out_pos + 1] = out_slice[(source_pos + 1) & out_buf_size_mask];
        }
        3 => {
            out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask];
            out_slice[out_pos + 1] = out_slice[(source_pos + 1) & out_buf_size_mask];
            out_slice[out_pos + 2] = out_slice[(source_pos + 2) & out_buf_size_mask];
        }
        _ => unreachable!(),
    }
}

/// Presumes that there is at least match_len bytes in output left.
#[inline]
fn apply_match(
    out_slice: &mut [u8],
    out_pos: usize,
    dist: usize,
    match_len: usize,
    out_buf_size_mask: usize,
) {
    debug_assert!(out_pos + match_len <= out_slice.len());

    let source_pos = out_pos.wrapping_sub(dist) & out_buf_size_mask;

    if match_len == 3 {
        // Fast path for match len 3.
        out_slice[out_pos] = out_slice[source_pos];
        out_slice[out_pos + 1] = out_slice[(source_pos + 1) & out_buf_size_mask];
        out_slice[out_pos + 2] = out_slice[(source_pos + 2) & out_buf_size_mask];
        return;
    }

    if cfg!(not(any(target_arch = "x86", target_arch = "x86_64"))) {
        // We are not on x86 so copy manually.
        transfer(out_slice, source_pos, out_pos, match_len, out_buf_size_mask);
        return;
    }

    if source_pos >= out_pos && (source_pos - out_pos) < match_len {
        transfer(out_slice, source_pos, out_pos, match_len, out_buf_size_mask);
    } else if match_len <= dist && source_pos + match_len < out_slice.len() {
        // Destination and source segments does not intersect and source does not wrap.
        if source_pos < out_pos {
            let (from_slice, to_slice) = out_slice.split_at_mut(out_pos);
            to_slice[..match_len].copy_from_slice(&from_slice[source_pos..source_pos + match_len]);
        } else {
            let (to_slice, from_slice) = out_slice.split_at_mut(source_pos);
            to_slice[out_pos..out_pos + match_len].copy_from_slice(&from_slice[..match_len]);
        }
    } else {
        transfer(out_slice, source_pos, out_pos, match_len, out_buf_size_mask);
    }
}

/// Fast inner decompression loop which is run  while there is at least
/// 259 bytes left in the output buffer, and at least 6 bytes left in the input buffer
/// (The maximum one match would need + 1).
///
/// This was inspired by a similar optimization in zlib, which uses this info to do
/// faster unchecked copies of multiple bytes at a time.
/// Currently we don't do this here, but this function does avoid having to jump through the
/// big match loop on each state change(as rust does not have fallthrough or gotos at the moment),
/// and already improves decompression speed a fair bit.
fn decompress_fast(
    r: &mut DecompressorOxide,
    in_iter: &mut slice::Iter<u8>,
    out_buf: &mut OutputBuffer,
    flags: u32,
    local_vars: &mut LocalVars,
    out_buf_size_mask: usize,
) -> (TINFLStatus, State) {
    // Make a local copy of the most used variables, to avoid having to update and read from values
    // in a random memory location and to encourage more register use.
    let mut l = *local_vars;
    let mut state;

    let status: TINFLStatus = 'o: loop {
        state = State::DecodeLitlen;
        loop {
            // This function assumes that there is at least 259 bytes left in the output buffer,
            // and that there is at least 14 bytes left in the input buffer. 14 input bytes:
            // 15 (prev lit) + 15 (length) + 5 (length extra) + 15 (dist)
            // + 29 + 32 (left in bit buf, including last 13 dist extra) = 111 bits < 14 bytes
            // We need the one extra byte as we may write one length and one full match
            // before checking again.
            if out_buf.bytes_left() < 259 || in_iter.len() < 14 {
                state = State::DecodeLitlen;
                break 'o TINFLStatus::Done;
            }

            fill_bit_buffer(&mut l, in_iter);

            if let Some((symbol, code_len)) = r.tables[LITLEN_TABLE].lookup(l.bit_buf) {
                l.counter = symbol as u32;
                l.bit_buf >>= code_len;
                l.num_bits -= code_len;

                if (l.counter & 256) != 0 {
                    // The symbol is not a literal.
                    break;
                } else {
                    // If we have a 32-bit buffer we need to read another two bytes now
                    // to have enough bits to keep going.
                    if cfg!(not(target_pointer_width = "64")) {
                        fill_bit_buffer(&mut l, in_iter);
                    }

                    if let Some((symbol, code_len)) = r.tables[LITLEN_TABLE].lookup(l.bit_buf) {
                        l.bit_buf >>= code_len;
                        l.num_bits -= code_len;
                        // The previous symbol was a literal, so write it directly and check
                        // the next one.
                        out_buf.write_byte(l.counter as u8);
                        if (symbol & 256) != 0 {
                            l.counter = symbol as u32;
                            // The symbol is a length value.
                            break;
                        } else {
                            // The symbol is a literal, so write it directly and continue.
                            out_buf.write_byte(symbol as u8);
                        }
                    } else {
                        state.begin(InvalidCodeLen);
                        break 'o TINFLStatus::Failed;
                    }
                }
            } else {
                state.begin(InvalidCodeLen);
                break 'o TINFLStatus::Failed;
            }
        }

        // Mask the top bits since they may contain length info.
        l.counter &= 511;
        if l.counter == 256 {
            // We hit the end of block symbol.
            state.begin(BlockDone);
            break 'o TINFLStatus::Done;
        } else if l.counter > 285 {
            // Invalid code.
            // We already verified earlier that the code is > 256.
            state.begin(InvalidLitlen);
            break 'o TINFLStatus::Failed;
        } else {
            // The symbol was a length code.
            // # Optimization
            // Mask the value to avoid bounds checks
            // We could use get_unchecked later if can statically verify that
            // this will never go out of bounds.
            l.num_extra = u32::from(LENGTH_EXTRA[(l.counter - 257) as usize & BASE_EXTRA_MASK]);
            l.counter = u32::from(LENGTH_BASE[(l.counter - 257) as usize & BASE_EXTRA_MASK]);
            // Length and distance codes have a number of extra bits depending on
            // the base, which together with the base gives us the exact value.

            fill_bit_buffer(&mut l, in_iter);
            if l.num_extra != 0 {
                let extra_bits = l.bit_buf & ((1 << l.num_extra) - 1);
                l.bit_buf >>= l.num_extra;
                l.num_bits -= l.num_extra;
                l.counter += extra_bits as u32;
            }

            // We found a length code, so a distance code should follow.

            if cfg!(not(target_pointer_width = "64")) {
                fill_bit_buffer(&mut l, in_iter);
            }

            if let Some((mut symbol, code_len)) = r.tables[DIST_TABLE].lookup(l.bit_buf) {
                symbol &= 511;
                l.bit_buf >>= code_len;
                l.num_bits -= code_len;
                if symbol > 29 {
                    state.begin(InvalidDist);
                    break 'o TINFLStatus::Failed;
                }

                l.num_extra = u32::from(DIST_EXTRA[symbol as usize]);
                l.dist = u32::from(DIST_BASE[symbol as usize]);
            } else {
                state.begin(InvalidCodeLen);
                break 'o TINFLStatus::Failed;
            }

            if l.num_extra != 0 {
                fill_bit_buffer(&mut l, in_iter);
                let extra_bits = l.bit_buf & ((1 << l.num_extra) - 1);
                l.bit_buf >>= l.num_extra;
                l.num_bits -= l.num_extra;
                l.dist += extra_bits as u32;
            }

            let position = out_buf.position();
            if l.dist as usize > out_buf.position()
                && (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0)
            {
                // We encountered a distance that refers a position before
                // the start of the decoded data, so we can't continue.
                state.begin(DistanceOutOfBounds);
                break TINFLStatus::Failed;
            }

            apply_match(
                out_buf.get_mut(),
                position,
                l.dist as usize,
                l.counter as usize,
                out_buf_size_mask,
            );

            out_buf.set_position(position + l.counter as usize);
        }
    };

    *local_vars = l;
    (status, state)
}

/// Main decompression function. Keeps decompressing data from `in_buf` until the `in_buf` is
/// empty, `out` is full, the end of the deflate stream is hit, or there is an error in the
/// deflate stream.
///
/// # Arguments
///
/// `r` is a [`DecompressorOxide`] struct with the state of this stream.
///
/// `in_buf` is a reference to the compressed data that is to be decompressed. The decompressor will
/// start at the first byte of this buffer.
///
/// `out` is a reference to the buffer that will store the decompressed data, and that
/// stores previously decompressed data if any.
///
/// * The offset given by `out_pos` indicates where in the output buffer slice writing should start.
/// * If [`TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF`] is not set, the output buffer is used in a
/// wrapping manner, and it's size is required to be a power of 2.
/// * The decompression function normally needs access to 32KiB of the previously decompressed data
///(or to the beginning of the decompressed data if less than 32KiB has been decompressed.)
///     - If this data is not available, decompression may fail.
///     - Some deflate compressors allow specifying a window size which limits match distances to
/// less than this, or alternatively an RLE mode where matches will only refer to the previous byte
/// and thus allows a smaller output buffer. The window size can be specified in the zlib
/// header structure, however, the header data should not be relied on to be correct.
///
/// `flags` indicates settings and status to the decompression function.
/// * The [`TINFL_FLAG_HAS_MORE_INPUT`] has to be specified if more compressed data is to be provided
/// in a subsequent call to this function.
/// * See the the [`inflate_flags`] module for details on other flags.
///
/// # Returns
///
/// Returns a tuple containing the status of the compressor, the number of input bytes read, and the
/// number of bytes output to `out`.
///
/// This function shouldn't panic pending any bugs.
pub fn decompress(
    r: &mut DecompressorOxide,
    in_buf: &[u8],
    out: &mut [u8],
    out_pos: usize,
    flags: u32,
) -> (TINFLStatus, usize, usize) {
    let out_buf_size_mask = if flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0 {
        usize::max_value()
    } else {
        // In the case of zero len, any attempt to write would produce HasMoreOutput,
        // so to gracefully process the case of there really being no output,
        // set the mask to all zeros.
        out.len().saturating_sub(1)
    };

    // Ensure the output buffer's size is a power of 2, unless the output buffer
    // is large enough to hold the entire output file (in which case it doesn't
    // matter).
    // Also make sure that the output buffer position is not past the end of the output buffer.
    if (out_buf_size_mask.wrapping_add(1) & out_buf_size_mask) != 0 || out_pos > out.len() {
        return (TINFLStatus::BadParam, 0, 0);
    }

    let mut in_iter = in_buf.iter();

    let mut state = r.state;

    let mut out_buf = OutputBuffer::from_slice_and_pos(out, out_pos);

    // Make a local copy of the important variables here so we can work with them on the stack.
    let mut l = LocalVars {
        bit_buf: r.bit_buf,
        num_bits: r.num_bits,
        dist: r.dist,
        counter: r.counter,
        num_extra: r.num_extra,
    };

    let mut status = 'state_machine: loop {
        match state {
            Start => generate_state!(state, 'state_machine, {
                l.bit_buf = 0;
                l.num_bits = 0;
                l.dist = 0;
                l.counter = 0;
                l.num_extra = 0;
                r.z_header0 = 0;
                r.z_header1 = 0;
                r.z_adler32 = 1;
                r.check_adler32 = 1;
                if flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                    Action::Jump(State::ReadZlibCmf)
                } else {
                    Action::Jump(State::ReadBlockHeader)
                }
            }),

            ReadZlibCmf => generate_state!(state, 'state_machine, {
                read_byte(&mut in_iter, flags, |cmf| {
                    r.z_header0 = u32::from(cmf);
                    Action::Jump(State::ReadZlibFlg)
                })
            }),

            ReadZlibFlg => generate_state!(state, 'state_machine, {
                read_byte(&mut in_iter, flags, |flg| {
                    r.z_header1 = u32::from(flg);
                    validate_zlib_header(r.z_header0, r.z_header1, flags, out_buf_size_mask)
                })
            }),

            // Read the block header and jump to the relevant section depending on the block type.
            ReadBlockHeader => generate_state!(state, 'state_machine, {
                read_bits(&mut l, 3, &mut in_iter, flags, |l, bits| {
                    r.finish = (bits & 1) as u32;
                    r.block_type = (bits >> 1) as u32 & 3;
                    match r.block_type {
                        0 => Action::Jump(BlockTypeNoCompression),
                        1 => {
                            start_static_table(r);
                            init_tree(r, l)
                        },
                        2 => {
                            l.counter = 0;
                            Action::Jump(ReadTableSizes)
                        },
                        3 => Action::Jump(BlockTypeUnexpected),
                        _ => unreachable!()
                    }
                })
            }),

            // Raw/Stored/uncompressed block.
            BlockTypeNoCompression => generate_state!(state, 'state_machine, {
                pad_to_bytes(&mut l, &mut in_iter, flags, |l| {
                    l.counter = 0;
                    Action::Jump(RawHeader)
                })
            }),

            // Check that the raw block header is correct.
            RawHeader => generate_state!(state, 'state_machine, {
                if l.counter < 4 {
                    // Read block length and block length check.
                    if l.num_bits != 0 {
                        read_bits(&mut l, 8, &mut in_iter, flags, |l, bits| {
                            r.raw_header[l.counter as usize] = bits as u8;
                            l.counter += 1;
                            Action::None
                        })
                    } else {
                        read_byte(&mut in_iter, flags, |byte| {
                            r.raw_header[l.counter as usize] = byte;
                            l.counter += 1;
                            Action::None
                        })
                    }
                } else {
                    // Check if the length value of a raw block is correct.
                    // The 2 first (2-byte) words in a raw header are the length and the
                    // ones complement of the length.
                    let length = u16::from(r.raw_header[0]) | (u16::from(r.raw_header[1]) << 8);
                    let check = u16::from(r.raw_header[2]) | (u16::from(r.raw_header[3]) << 8);
                    let valid = length == !check;
                    l.counter = length.into();

                    if !valid {
                        Action::Jump(BadRawLength)
                    } else if l.counter == 0 {
                        // Empty raw block. Sometimes used for synchronization.
                        Action::Jump(BlockDone)
                    } else if l.num_bits != 0 {
                        // There is some data in the bit buffer, so we need to write that first.
                        Action::Jump(RawReadFirstByte)
                    } else {
                        // The bit buffer is empty, so memcpy the rest of the uncompressed data from
                        // the block.
                        Action::Jump(RawMemcpy1)
                    }
                }
            }),

            // Read the byte from the bit buffer.
            RawReadFirstByte => generate_state!(state, 'state_machine, {
                read_bits(&mut l, 8, &mut in_iter, flags, |l, bits| {
                    l.dist = bits as u32;
                    Action::Jump(RawStoreFirstByte)
                })
            }),

            // Write the byte we just read to the output buffer.
            RawStoreFirstByte => generate_state!(state, 'state_machine, {
                if out_buf.bytes_left() == 0 {
                    Action::End(TINFLStatus::HasMoreOutput)
                } else {
                    out_buf.write_byte(l.dist as u8);
                    l.counter -= 1;
                    if l.counter == 0 || l.num_bits == 0 {
                        Action::Jump(RawMemcpy1)
                    } else {
                        // There is still some data left in the bit buffer that needs to be output.
                        // TODO: Changed this to jump to `RawReadfirstbyte` rather than
                        // `RawStoreFirstByte` as that seemed to be the correct path, but this
                        // needs testing.
                        Action::Jump(RawReadFirstByte)
                    }
                }
            }),

            RawMemcpy1 => generate_state!(state, 'state_machine, {
                if l.counter == 0 {
                    Action::Jump(BlockDone)
                } else if out_buf.bytes_left() == 0 {
                    Action::End(TINFLStatus::HasMoreOutput)
                } else {
                    Action::Jump(RawMemcpy2)
                }
            }),

            RawMemcpy2 => generate_state!(state, 'state_machine, {
                if in_iter.len() > 0 {
                    // Copy as many raw bytes as possible from the input to the output using memcpy.
                    // Raw block lengths are limited to 64 * 1024, so casting through usize and u32
                    // is not an issue.
                    let space_left = out_buf.bytes_left();
                    let bytes_to_copy = cmp::min(cmp::min(
                        space_left,
                        in_iter.len()),
                        l.counter as usize
                    );

                    out_buf.write_slice(&in_iter.as_slice()[..bytes_to_copy]);

                    (&mut in_iter).nth(bytes_to_copy - 1);
                    l.counter -= bytes_to_copy as u32;
                    Action::Jump(RawMemcpy1)
                } else {
                    end_of_input(flags)
                }
            }),

            // Read how many huffman codes/symbols are used for each table.
            ReadTableSizes => generate_state!(state, 'state_machine, {
                if l.counter < 3 {
                    let num_bits = [5, 5, 4][l.counter as usize];
                    read_bits(&mut l, num_bits, &mut in_iter, flags, |l, bits| {
                        r.table_sizes[l.counter as usize] =
                            bits as u32 + u32::from(MIN_TABLE_SIZES[l.counter as usize]);
                        l.counter += 1;
                        Action::None
                    })
                } else {
                    memset(&mut r.tables[HUFFLEN_TABLE].code_size[..], 0);
                    l.counter = 0;
                    // Check that the litlen and distance are within spec.
                    // litlen table should be <=286 acc to the RFC and
                    // additionally zlib rejects dist table sizes larger than 30.
                    // NOTE this the final sizes after adding back predefined values, not
                    // raw value in the data.
                    // See miniz_oxide issue #130 and https://github.com/madler/zlib/issues/82.
                    if r.table_sizes[LITLEN_TABLE] <= 286 && r.table_sizes[DIST_TABLE] <= 30 {
                        Action::Jump(ReadHufflenTableCodeSize)
                    }
                    else {
                        Action::Jump(BadDistOrLiteralTableLength)
                    }
                }
            }),

            // Read the 3-bit lengths of the huffman codes describing the huffman code lengths used
            // to decode the lengths of the main tables.
            ReadHufflenTableCodeSize => generate_state!(state, 'state_machine, {
                if l.counter < r.table_sizes[HUFFLEN_TABLE] {
                    read_bits(&mut l, 3, &mut in_iter, flags, |l, bits| {
                        // These lengths are not stored in a normal ascending order, but rather one
                        // specified by the deflate specification intended to put the most used
                        // values at the front as trailing zero lengths do not have to be stored.
                        r.tables[HUFFLEN_TABLE]
                            .code_size[HUFFMAN_LENGTH_ORDER[l.counter as usize] as usize] =
                                bits as u8;
                        l.counter += 1;
                        Action::None
                    })
                } else {
                    r.table_sizes[HUFFLEN_TABLE] = 19;
                    init_tree(r, &mut l)
                }
            }),

            ReadLitlenDistTablesCodeSize => generate_state!(state, 'state_machine, {
                if l.counter < r.table_sizes[LITLEN_TABLE] + r.table_sizes[DIST_TABLE] {
                    decode_huffman_code(
                        r, &mut l, HUFFLEN_TABLE,
                        flags, &mut in_iter, |r, l, symbol| {
                            l.dist = symbol as u32;
                            if l.dist < 16 {
                                r.len_codes[l.counter as usize] = l.dist as u8;
                                l.counter += 1;
                                Action::None
                            } else if l.dist == 16 && l.counter == 0 {
                                Action::Jump(BadCodeSizeDistPrevLookup)
                            } else {
                                l.num_extra = [2, 3, 7][l.dist as usize - 16];
                                Action::Jump(ReadExtraBitsCodeSize)
                            }
                        }
                    )
                } else if l.counter != r.table_sizes[LITLEN_TABLE] + r.table_sizes[DIST_TABLE] {
                    Action::Jump(BadCodeSizeSum)
                } else {
                    r.tables[LITLEN_TABLE].code_size[..r.table_sizes[LITLEN_TABLE] as usize]
                        .copy_from_slice(&r.len_codes[..r.table_sizes[LITLEN_TABLE] as usize]);

                    let dist_table_start = r.table_sizes[LITLEN_TABLE] as usize;
                    let dist_table_end = (r.table_sizes[LITLEN_TABLE] +
                                          r.table_sizes[DIST_TABLE]) as usize;
                    r.tables[DIST_TABLE].code_size[..r.table_sizes[DIST_TABLE] as usize]
                        .copy_from_slice(&r.len_codes[dist_table_start..dist_table_end]);

                    r.block_type -= 1;
                    init_tree(r, &mut l)
                }
            }),

            ReadExtraBitsCodeSize => generate_state!(state, 'state_machine, {
                let num_extra = l.num_extra;
                read_bits(&mut l, num_extra, &mut in_iter, flags, |l, mut extra_bits| {
                    // Mask to avoid a bounds check.
                    extra_bits += [3, 3, 11][(l.dist as usize - 16) & 3];
                    let val = if l.dist == 16 {
                        r.len_codes[l.counter as usize - 1]
                    } else {
                        0
                    };

                    memset(
                        &mut r.len_codes[
                            l.counter as usize..l.counter as usize + extra_bits as usize
                        ],
                        val,
                    );
                    l.counter += extra_bits as u32;
                    Action::Jump(ReadLitlenDistTablesCodeSize)
                })
            }),

            DecodeLitlen => generate_state!(state, 'state_machine, {
                if in_iter.len() < 4 || out_buf.bytes_left() < 2 {
                    // See if we can decode a literal with the data we have left.
                    // Jumps to next state (WriteSymbol) if successful.
                    decode_huffman_code(
                        r,
                        &mut l,
                        LITLEN_TABLE,
                        flags,
                        &mut in_iter,
                        |_r, l, symbol| {
                            l.counter = symbol as u32;
                            Action::Jump(WriteSymbol)
                        },
                    )
                } else if
                // If there is enough space, use the fast inner decompression
                // function.
                    out_buf.bytes_left() >= 259 &&
                    in_iter.len() >= 14
                {
                    let (status, new_state) = decompress_fast(
                        r,
                        &mut in_iter,
                        &mut out_buf,
                        flags,
                        &mut l,
                        out_buf_size_mask,
                    );

                    state = new_state;
                    if status == TINFLStatus::Done {
                        Action::Jump(new_state)
                    } else {
                        Action::End(status)
                    }
                } else {
                    fill_bit_buffer(&mut l, &mut in_iter);

                    if let Some((symbol, code_len)) = r.tables[LITLEN_TABLE].lookup(l.bit_buf) {

                    l.counter = symbol as u32;
                    l.bit_buf >>= code_len;
                    l.num_bits -= code_len;

                    if (l.counter & 256) != 0 {
                        // The symbol is not a literal.
                        Action::Jump(HuffDecodeOuterLoop1)
                    } else {
                        // If we have a 32-bit buffer we need to read another two bytes now
                        // to have enough bits to keep going.
                        if cfg!(not(target_pointer_width = "64")) {
                            fill_bit_buffer(&mut l, &mut in_iter);
                        }

                        if let Some((symbol, code_len)) = r.tables[LITLEN_TABLE].lookup(l.bit_buf) {

                            l.bit_buf >>= code_len;
                            l.num_bits -= code_len;
                            // The previous symbol was a literal, so write it directly and check
                            // the next one.
                            out_buf.write_byte(l.counter as u8);
                            if (symbol & 256) != 0 {
                                l.counter = symbol as u32;
                                // The symbol is a length value.
                                Action::Jump(HuffDecodeOuterLoop1)
                            } else {
                                // The symbol is a literal, so write it directly and continue.
                                out_buf.write_byte(symbol as u8);
                                Action::None
                            }
                        } else {
                            Action::Jump(InvalidCodeLen)
                        }
                    }
                    } else {
                        Action::Jump(InvalidCodeLen)
                    }
                }
            }),

            WriteSymbol => generate_state!(state, 'state_machine, {
                if l.counter >= 256 {
                    Action::Jump(HuffDecodeOuterLoop1)
                } else if out_buf.bytes_left() > 0 {
                    out_buf.write_byte(l.counter as u8);
                    Action::Jump(DecodeLitlen)
                } else {
                    Action::End(TINFLStatus::HasMoreOutput)
                }
            }),

            HuffDecodeOuterLoop1 => generate_state!(state, 'state_machine, {
                // Mask the top bits since they may contain length info.
                l.counter &= 511;

                if l.counter
                    == 256 {
                    // We hit the end of block symbol.
                    Action::Jump(BlockDone)
                } else if l.counter > 285 {
                    // Invalid code.
                    // We already verified earlier that the code is > 256.
                    Action::Jump(InvalidLitlen)
                } else {
                    // # Optimization
                    // Mask the value to avoid bounds checks
                    // We could use get_unchecked later if can statically verify that
                    // this will never go out of bounds.
                    l.num_extra =
                        u32::from(LENGTH_EXTRA[(l.counter - 257) as usize & BASE_EXTRA_MASK]);
                    l.counter = u32::from(LENGTH_BASE[(l.counter - 257) as usize & BASE_EXTRA_MASK]);
                    // Length and distance codes have a number of extra bits depending on
                    // the base, which together with the base gives us the exact value.
                    if l.num_extra != 0 {
                        Action::Jump(ReadExtraBitsLitlen)
                    } else {
                        Action::Jump(DecodeDistance)
                    }
                }
            }),

            ReadExtraBitsLitlen => generate_state!(state, 'state_machine, {
                let num_extra = l.num_extra;
                read_bits(&mut l, num_extra, &mut in_iter, flags, |l, extra_bits| {
                    l.counter += extra_bits as u32;
                    Action::Jump(DecodeDistance)
                })
            }),

            DecodeDistance => generate_state!(state, 'state_machine, {
                // Try to read a huffman code from the input buffer and look up what
                // length code the decoded symbol refers to.
                decode_huffman_code(r, &mut l, DIST_TABLE, flags, &mut in_iter, |_r, l, symbol| {
                    if symbol > 29 {
                        // Invalid distance code.
                        return Action::Jump(InvalidDist)
                    }
                    // # Optimization
                    // Mask the value to avoid bounds checks
                    // We could use get_unchecked later if can statically verify that
                    // this will never go out of bounds.
                    l.num_extra = u32::from(DIST_EXTRA[symbol as usize & BASE_EXTRA_MASK]);
                    l.dist = u32::from(DIST_BASE[symbol as usize & BASE_EXTRA_MASK]);
                    if l.num_extra != 0 {
                        // ReadEXTRA_BITS_DISTACNE
                        Action::Jump(ReadExtraBitsDistance)
                    } else {
                        Action::Jump(HuffDecodeOuterLoop2)
                    }
                })
            }),

            ReadExtraBitsDistance => generate_state!(state, 'state_machine, {
                let num_extra = l.num_extra;
                read_bits(&mut l, num_extra, &mut in_iter, flags, |l, extra_bits| {
                    l.dist += extra_bits as u32;
                    Action::Jump(HuffDecodeOuterLoop2)
                })
            }),

            HuffDecodeOuterLoop2 => generate_state!(state, 'state_machine, {
                if l.dist as usize > out_buf.position() &&
                    (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0)
                {
                    // We encountered a distance that refers a position before
                    // the start of the decoded data, so we can't continue.
                    Action::Jump(DistanceOutOfBounds)
                } else {
                    let out_pos = out_buf.position();
                    let source_pos = out_buf.position()
                        .wrapping_sub(l.dist as usize) & out_buf_size_mask;

                    let out_len = out_buf.get_ref().len() as usize;
                    let match_end_pos = out_buf.position() + l.counter as usize;

                    if match_end_pos > out_len ||
                        // miniz doesn't do this check here. Not sure how it makes sure
                        // that this case doesn't happen.
                        (source_pos >= out_pos && (source_pos - out_pos) < l.counter as usize)
                    {
                        // Not enough space for all of the data in the output buffer,
                        // so copy what we have space for.
                        if l.counter == 0 {
                            Action::Jump(DecodeLitlen)
                        } else {
                            Action::Jump(WriteLenBytesToEnd)
                        }
                    } else {
                        apply_match(
                            out_buf.get_mut(),
                            out_pos,
                            l.dist as usize,
                            l.counter as usize,
                            out_buf_size_mask
                        );
                        out_buf.set_position(out_pos + l.counter as usize);
                        Action::Jump(DecodeLitlen)
                    }
                }
            }),

            WriteLenBytesToEnd => generate_state!(state, 'state_machine, {
                if out_buf.bytes_left() > 0 {
                    let out_pos = out_buf.position();
                    let source_pos = out_buf.position()
                        .wrapping_sub(l.dist as usize) & out_buf_size_mask;


                    let len = cmp::min(out_buf.bytes_left(), l.counter as usize);

                    transfer(out_buf.get_mut(), source_pos, out_pos, len, out_buf_size_mask);

                    out_buf.set_position(out_pos + len);
                    l.counter -= len as u32;
                    if l.counter == 0 {
                        Action::Jump(DecodeLitlen)
                    } else {
                        Action::None
                    }
                } else {
                    Action::End(TINFLStatus::HasMoreOutput)
                }
            }),

            BlockDone => generate_state!(state, 'state_machine, {
                // End once we've read the last block.
                if r.finish != 0 {
                    pad_to_bytes(&mut l, &mut in_iter, flags, |_| Action::None);

                    let in_consumed = in_buf.len() - in_iter.len();
                    let undo = undo_bytes(&mut l, in_consumed as u32) as usize;
                    in_iter = in_buf[in_consumed - undo..].iter();

                    l.bit_buf &= ((1 as BitBuffer) << l.num_bits) - 1;
                    debug_assert_eq!(l.num_bits, 0);

                    if flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                        l.counter = 0;
                        Action::Jump(ReadAdler32)
                    } else {
                        Action::Jump(DoneForever)
                    }
                } else {
                    Action::Jump(ReadBlockHeader)
                }
            }),

            ReadAdler32 => generate_state!(state, 'state_machine, {
                if l.counter < 4 {
                    if l.num_bits != 0 {
                        read_bits(&mut l, 8, &mut in_iter, flags, |l, bits| {
                            r.z_adler32 <<= 8;
                            r.z_adler32 |= bits as u32;
                            l.counter += 1;
                            Action::None
                        })
                    } else {
                        read_byte(&mut in_iter, flags, |byte| {
                            r.z_adler32 <<= 8;
                            r.z_adler32 |= u32::from(byte);
                            l.counter += 1;
                            Action::None
                        })
                    }
                } else {
                    Action::Jump(DoneForever)
                }
            }),

            // We are done.
            DoneForever => break TINFLStatus::Done,

            // Anything else indicates failure.
            // BadZlibHeader | BadRawLength | BadDistOrLiteralTableLength | BlockTypeUnexpected |
            // DistanceOutOfBounds |
            // BadTotalSymbols | BadCodeSizeDistPrevLookup | BadCodeSizeSum | InvalidLitlen |
            // InvalidDist | InvalidCodeLen
            _ => break TINFLStatus::Failed,
        };
    };

    let in_undo = if status != TINFLStatus::NeedsMoreInput
        && status != TINFLStatus::FailedCannotMakeProgress
    {
        undo_bytes(&mut l, (in_buf.len() - in_iter.len()) as u32) as usize
    } else {
        0
    };

    // Make sure HasMoreOutput overrides NeedsMoreInput if the output buffer is full.
    // (Unless the missing input is the adler32 value in which case we don't need to write anything.)
    // TODO: May want to see if we can do this in a better way.
    if status == TINFLStatus::NeedsMoreInput
        && out_buf.bytes_left() == 0
        && state != State::ReadAdler32
    {
        status = TINFLStatus::HasMoreOutput
    }

    r.state = state;
    r.bit_buf = l.bit_buf;
    r.num_bits = l.num_bits;
    r.dist = l.dist;
    r.counter = l.counter;
    r.num_extra = l.num_extra;

    r.bit_buf &= ((1 as BitBuffer) << r.num_bits) - 1;

    // If this is a zlib stream, and update the adler32 checksum with the decompressed bytes if
    // requested.
    let need_adler = if (flags & TINFL_FLAG_IGNORE_ADLER32) == 0 {
        flags & (TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_COMPUTE_ADLER32) != 0
    } else {
        // If TINFL_FLAG_IGNORE_ADLER32 is enabled, ignore the checksum.
        false
    };
    if need_adler && status as i32 >= 0 {
        let out_buf_pos = out_buf.position();
        r.check_adler32 = update_adler32(r.check_adler32, &out_buf.get_ref()[out_pos..out_buf_pos]);

        // disabled so that random input from fuzzer would not be rejected early,
        // before it has a chance to reach interesting parts of code
        if !cfg!(fuzzing) {
            // Once we are done, check if the checksum matches with the one provided in the zlib header.
            if status == TINFLStatus::Done
                && flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0
                && r.check_adler32 != r.z_adler32
            {
                status = TINFLStatus::Adler32Mismatch;
            }
        }
    }

    (
        status,
        in_buf.len() - in_iter.len() - in_undo,
        out_buf.position() - out_pos,
    )
}

#[cfg(test)]
mod test {
    use super::*;

    //TODO: Fix these.

    fn tinfl_decompress_oxide<'i>(
        r: &mut DecompressorOxide,
        input_buffer: &'i [u8],
        output_buffer: &mut [u8],
        flags: u32,
    ) -> (TINFLStatus, &'i [u8], usize) {
        let (status, in_pos, out_pos) = decompress(r, input_buffer, output_buffer, 0, flags);
        (status, &input_buffer[in_pos..], out_pos)
    }

    #[test]
    fn decompress_zlib() {
        let encoded = [
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
        ];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;

        let mut b = DecompressorOxide::new();
        const LEN: usize = 32;
        let mut b_buf = vec![0; LEN];

        // This should fail with the out buffer being to small.
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);

        assert_eq!(b_status.0, TINFLStatus::Failed);

        let flags = flags | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        b = DecompressorOxide::new();

        // With TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF set this should no longer fail.
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);

        assert_eq!(b_buf[..b_status.2], b"Hello, zlib!"[..]);
        assert_eq!(b_status.0, TINFLStatus::Done);
    }

    #[test]
    fn raw_block() {
        const LEN: usize = 64;

        let text = b"Hello, zlib!";
        let encoded = {
            let len = text.len();
            let notlen = !len;
            let mut encoded = vec![
                1,
                len as u8,
                (len >> 8) as u8,
                notlen as u8,
                (notlen >> 8) as u8,
            ];
            encoded.extend_from_slice(&text[..]);
            encoded
        };

        //let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER |
        let flags = TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        let mut b = DecompressorOxide::new();

        let mut b_buf = vec![0; LEN];

        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);
        assert_eq!(b_buf[..b_status.2], text[..]);
        assert_eq!(b_status.0, TINFLStatus::Done);
    }

    fn masked_lookup(table: &HuffmanTable, bit_buf: BitBuffer) -> (i32, u32) {
        let ret = table.lookup(bit_buf).unwrap();
        (ret.0 & 511, ret.1)
    }

    #[test]
    fn fixed_table_lookup() {
        let mut d = DecompressorOxide::new();
        d.block_type = 1;
        start_static_table(&mut d);
        let mut l = LocalVars {
            bit_buf: d.bit_buf,
            num_bits: d.num_bits,
            dist: d.dist,
            counter: d.counter,
            num_extra: d.num_extra,
        };
        init_tree(&mut d, &mut l);
        let llt = &d.tables[LITLEN_TABLE];
        let dt = &d.tables[DIST_TABLE];
        assert_eq!(masked_lookup(llt, 0b00001100), (0, 8));
        assert_eq!(masked_lookup(llt, 0b00011110), (72, 8));
        assert_eq!(masked_lookup(llt, 0b01011110), (74, 8));
        assert_eq!(masked_lookup(llt, 0b11111101), (143, 8));
        assert_eq!(masked_lookup(llt, 0b000010011), (144, 9));
        assert_eq!(masked_lookup(llt, 0b111111111), (255, 9));
        assert_eq!(masked_lookup(llt, 0b00000000), (256, 7));
        assert_eq!(masked_lookup(llt, 0b1110100), (279, 7));
        assert_eq!(masked_lookup(llt, 0b00000011), (280, 8));
        assert_eq!(masked_lookup(llt, 0b11100011), (287, 8));

        assert_eq!(masked_lookup(dt, 0), (0, 5));
        assert_eq!(masked_lookup(dt, 20), (5, 5));
    }

    fn check_result(input: &[u8], expected_status: TINFLStatus, expected_state: State, zlib: bool) {
        let mut r = DecompressorOxide::default();
        let mut output_buf = vec![0; 1024 * 32];
        let flags = if zlib {
            inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
        } else {
            0
        } | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
            | TINFL_FLAG_HAS_MORE_INPUT;
        let (d_status, _in_bytes, _out_bytes) =
            decompress(&mut r, input, &mut output_buf, 0, flags);
        assert_eq!(expected_status, d_status);
        assert_eq!(expected_state, r.state);
    }

    #[test]
    fn bogus_input() {
        use self::check_result as cr;
        const F: TINFLStatus = TINFLStatus::Failed;
        const OK: TINFLStatus = TINFLStatus::Done;
        // Bad CM.
        cr(&[0x77, 0x85], F, State::BadZlibHeader, true);
        // Bad window size (but check is correct).
        cr(&[0x88, 0x98], F, State::BadZlibHeader, true);
        // Bad check bits.
        cr(&[0x78, 0x98], F, State::BadZlibHeader, true);

        // Too many code lengths. (From inflate library issues)
        cr(
            b"M\xff\xffM*\xad\xad\xad\xad\xad\xad\xad\xcd\xcd\xcdM",
            F,
            State::BadDistOrLiteralTableLength,
            false,
        );

        // Bad CLEN (also from inflate library issues)
        cr(
            b"\xdd\xff\xff*M\x94ffffffffff",
            F,
            State::BadDistOrLiteralTableLength,
            false,
        );

        // Port of inflate coverage tests from zlib-ng
        // https://github.com/Dead2/zlib-ng/blob/develop/test/infcover.c
        let c = |a, b, c| cr(a, b, c, false);

        // Invalid uncompressed/raw block length.
        c(&[0, 0, 0, 0, 0], F, State::BadRawLength);
        // Ok empty uncompressed block.
        c(&[3, 0], OK, State::DoneForever);
        // Invalid block type.
        c(&[6], F, State::BlockTypeUnexpected);
        // Ok uncompressed block.
        c(&[1, 1, 0, 0xfe, 0xff, 0], OK, State::DoneForever);
        // Too many litlens, we handle this later than zlib, so this test won't
        // give the same result.
        //        c(&[0xfc, 0, 0], F, State::BadTotalSymbols);
        // Invalid set of code lengths - TODO Check if this is the correct error for this.
        c(&[4, 0, 0xfe, 0xff], F, State::BadTotalSymbols);
        // Invalid repeat in list of code lengths.
        // (Try to repeat a non-existent code.)
        c(&[4, 0, 0x24, 0x49, 0], F, State::BadCodeSizeDistPrevLookup);
        // Missing end of block code (should we have a separate error for this?) - fails on further input
        //    c(&[4, 0, 0x24, 0xe9, 0xff, 0x6d], F, State::BadTotalSymbols);
        // Invalid set of literals/lengths
        c(
            &[
                4, 0x80, 0x49, 0x92, 0x24, 0x49, 0x92, 0x24, 0x71, 0xff, 0xff, 0x93, 0x11, 0,
            ],
            F,
            State::BadTotalSymbols,
        );
        // Invalid set of distances _ needsmoreinput
        // c(&[4, 0x80, 0x49, 0x92, 0x24, 0x49, 0x92, 0x24, 0x0f, 0xb4, 0xff, 0xff, 0xc3, 0x84], F, State::BadTotalSymbols);
        // Invalid distance code
        c(&[2, 0x7e, 0xff, 0xff], F, State::InvalidDist);

        // Distance refers to position before the start
        c(
            &[0x0c, 0xc0, 0x81, 0, 0, 0, 0, 0, 0x90, 0xff, 0x6b, 0x4, 0],
            F,
            State::DistanceOutOfBounds,
        );

        // Trailer
        // Bad gzip trailer checksum GZip header not handled by miniz_oxide
        //cr(&[0x1f, 0x8b, 0x08 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0x03, 0, 0, 0, 0, 0x01], F, State::BadCRC, false)
        // Bad gzip trailer length
        //cr(&[0x1f, 0x8b, 0x08 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0x01], F, State::BadCRC, false)
    }

    #[test]
    fn empty_output_buffer_non_wrapping() {
        let encoded = [
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
        ];
        let flags = TINFL_FLAG_COMPUTE_ADLER32
            | TINFL_FLAG_PARSE_ZLIB_HEADER
            | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
        let mut r = DecompressorOxide::new();
        let mut output_buf = vec![];
        // Check that we handle an empty buffer properly and not panicking.
        // https://github.com/Frommi/miniz_oxide/issues/23
        let res = decompress(&mut r, &encoded, &mut output_buf, 0, flags);
        assert_eq!(res, (TINFLStatus::HasMoreOutput, 4, 0));
    }

    #[test]
    fn empty_output_buffer_wrapping() {
        let encoded = [
            0x73, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x00,
        ];
        let flags = TINFL_FLAG_COMPUTE_ADLER32;
        let mut r = DecompressorOxide::new();
        let mut output_buf = vec![];
        // Check that we handle an empty buffer properly and not panicking.
        // https://github.com/Frommi/miniz_oxide/issues/23
        let res = decompress(&mut r, &encoded, &mut output_buf, 0, flags);
        assert_eq!(res, (TINFLStatus::HasMoreOutput, 2, 0));
    }
}
