use super::*;
use lib_oxide::update_adler32;

use std::{cmp, slice, ptr};

use self::output_buffer::OutputBuffer;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(C)]
pub enum State {
    Start,
    ReadZlibCmf,
    ReadZlibFlg,
    ReadBlockHeader,
    BlockTypeNoCompression,
    RawHeader1,
    RawHeader2,
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
    BadTotalSymbols,
    BadZlibHeader,
    DistanceOutOfBounds,
    BadRawLength,
    BadCodeSizeDistPrevLookup,
}

use self::State::*;

// Not sure why miniz uses 32-bit values for these, maybe alignment/cache again?
// # Optimization
// We add a extra zero value at the end and make the tables 32 elements long
// so we can use a mask to avoid bounds checks.
/// Base length for each length code.
///
/// The base is used together with the value of the extra bits to decode the actual
/// length/distance values in a match.
const LENGTH_BASE: [u16; 32] = [
    3,  4,  5,  6,  7,  8,  9,  10,  11,  13,  15,  17,  19,  23, 27, 31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 0,  0, 0
];

/// Number of extra bits for each length code.
const LENGTH_EXTRA: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1,
    1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4,
    4, 4, 5, 5, 5, 5, 0, 0, 0, 0
];

/// Base length for each distance code.
const DIST_BASE: [u16; 32] = [
    1,    2,    3,    4,    5,    7,     9,     13,    17,  25,   33,
    49,   65,   97,   129,  193,  257,   385,   513,   769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0,   0
];

/// Number of extra bits for each distance code.
const DIST_EXTRA: [u8; 32] = [
    0, 0, 0,  0,  1,  1,  2,  2,  3,  3,
    4, 4, 5,  5,  6,  6,  7,  7,  8,  8,
    9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
    13, 13
];

/// The mask used when indexing the base/extra arrays.
const BASE_EXTRA_MASK: usize = 32 - 1;

/// Sets the value of all the elements of the slice to `val`.
#[inline]
pub fn memset<T : Copy>(slice: &mut [T], val: T) {
    for x in slice { *x = val }
}

/// Read an le u16 value from the slice iterator.
///
/// # Panics
/// Panics if there are less than two bytes left.
fn read_u16_le(iter: &mut slice::Iter<u8>) -> u16 {
    let ret = {
        let two_bytes = &iter.as_ref()[0..2];
        // # Unsafe
        //
        // The slice was just bounds checked to be 2 bytes long.
        unsafe {
            ptr::read_unaligned(two_bytes.as_ptr() as *const u16)
        }
    };
    iter.nth(1);
    u16::from_le(ret)
}

/// Read an le u32 value from the slice iterator.
///
/// # Panics
/// Panics if there are less than four bytes left.
fn read_u32_le(iter: &mut slice::Iter<u8>) -> u32 {
    let ret = {
        let four_bytes = &iter.as_ref()[..4];
        // # Unsafe
        //
        // The slice was just bounds checked to be 4 bytes long.
        unsafe {
            ptr::read_unaligned(four_bytes.as_ptr() as *const u32)
        }
    };
    iter.nth(3);
    u32::from_le(ret)
}

#[inline]
fn _transfer_unaligned_u64(buf: &mut &mut[u8], from: isize, to: isize) {
    unsafe {
        let mut data = ptr::read_unaligned((*buf).as_ptr().offset(from) as *const u32);
        ptr::write_unaligned((*buf).as_mut_ptr().offset(to) as *mut u32, data);

        data = ptr::read_unaligned((*buf).as_ptr().offset(from + 4) as *const u32);
        ptr::write_unaligned((*buf).as_mut_ptr().offset(to + 4) as *mut u32, data);
    };
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
        ((flg & 0b000100000) != 0) ||
    // Compression method. Only 8(DEFLATE) is defined by the standard.
        ((cmf & 15) != 8);

    if (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF) == 0 {
        let window_size = 1 << (8 + cmf >> 4);
        // Zlib doesn't allow window size above 32 * 1024.
        // Also bail if the buffer is wrapping and the window size is larger than the buffer.
        failed |= (
            window_size > 32768) ||
            ((mask + 1) < window_size);
    }

    if failed {
        Action::Jump(BadZlibHeader)
    } else {
        Action::Jump(ReadBlockHeader)
    }
}


pub enum Action {
    None,
    Jump(State),
    End(TINFLStatus),
}

/// Try to decode the next huffman code, and puts it in the counter field of the decompressor
/// if successful.
///
/// # Returns
/// The specified action returned from `f` on success,
/// Action::End if there are not enough data left to decode a symbol.
fn decode_huffman_code<F>(
    r: &mut tinfl_decompressor,
    l: &mut LocalVars,
    table: usize,
    flags: u32,
    in_iter: &mut slice::Iter<u8>,
    f: F,
) -> Action
    where F: FnOnce(&mut tinfl_decompressor, &mut LocalVars, i32) -> Action,
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
                let mut temp = r.tables[table].fast_lookup(l.bit_buf) as i32;

                if temp >= 0 {
                    let code_len = (temp >> 9) as u32;
                    if (code_len != 0) && (l.num_bits >= code_len) {
                        break;
                    }
                } else if l.num_bits > TINFL_FAST_LOOKUP_BITS.into() {
                    let mut code_len = TINFL_FAST_LOOKUP_BITS as u32;
                    loop {
                        temp = r.tables[table].tree[
                            (!temp + ((l.bit_buf >> code_len) & 1) as i32) as usize] as i32;
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
                match read_byte(in_iter, flags, |b| {
                    byte = b;
                    Action::None
                }) {
                    a @ Action::End(_) => return a,
                    _ => (),
                };

                // Do this outside closure for now to avoid borrowing r.
                l.bit_buf |= (byte as BitBuffer) << l.num_bits;
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
            let b0 = *in_iter.next().unwrap() as BitBuffer;
            let b1 = *in_iter.next().unwrap() as BitBuffer;

            l.bit_buf |= (b0 << l.num_bits) | (b1 << l.num_bits + 8);
            l.num_bits += 16;
        }
    }

    // We now have at least 15 bits in the input buffer.
    let mut symbol = r.tables[table].fast_lookup(l.bit_buf) as i32;
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
        let res = r.tables[table].tree_lookup(symbol, l.bit_buf, TINFL_FAST_LOOKUP_BITS as u32);
        symbol = res.0;
        code_len = res.1 as u32;
    };

    l.bit_buf >>= code_len as u32;
    l.num_bits -= code_len;
    f(r, l, symbol)
}

#[inline]
fn read_byte<F>(in_iter:  &mut slice::Iter<u8>, flags: u32, f: F) -> Action
    where F: FnOnce(u8) -> Action,
{
    match in_iter.next() {
        None => end_of_input(flags),
        Some(&byte) => {
            f(byte)
        }
    }
}

#[inline]
fn read_bits<F>(
    l: &mut LocalVars,
    amount: u32,
    in_iter: &mut slice::Iter<u8>,
    flags: u32,
    f: F,
) -> Action
    where F: FnOnce(&mut LocalVars, BitBuffer) -> Action,
{
    while l.num_bits < amount {
        match read_byte(in_iter, flags, |byte| {
            l.bit_buf |= (byte as BitBuffer) << l.num_bits;
            l.num_bits += 8;
            Action::None
        }) {
            Action::None => (),
            action => return action,
        }
    }

    let bits = l.bit_buf & ((1 << amount) - 1);
    l.bit_buf >>= amount;
    l.num_bits -= amount;
    f(l, bits)
}

#[inline]
fn pad_to_bytes<F>(
    l: &mut LocalVars,
    in_iter: &mut slice::Iter<u8>,
    flags: u32,
    f: F,
) -> Action
    where F: FnOnce(&mut LocalVars) -> Action,
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
    let res = cmp::min((l.num_bits >> 3), max);
    l.num_bits -= res << 3;
    res
}

fn start_static_table(r: &mut tinfl_decompressor) {
    r.table_sizes[LITLEN_TABLE] = 288;
    r.table_sizes[DIST_TABLE] = 32;
    memset(&mut r.tables[LITLEN_TABLE].code_size[0..144], 8);
    memset(&mut r.tables[LITLEN_TABLE].code_size[144..256], 9);
    memset(&mut r.tables[LITLEN_TABLE].code_size[256..280], 7);
    memset(&mut r.tables[LITLEN_TABLE].code_size[280..288], 8);
    memset(&mut r.tables[DIST_TABLE].code_size[0..32], 5);
}

fn init_tree(r: &mut tinfl_decompressor, l: &mut LocalVars) -> Action {
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
        let mut used_symbols = 0;
        let mut total = 0;
        for i in 1..16 {
            used_symbols += total_symbols[i];
            total += total_symbols[i];
            total <<= 1;
            next_code[i + 1] = total;
        }

        if total != 65536 && used_symbols > 1 {
            return Action::Jump(BadTotalSymbols);
        }

        let mut tree_next = -1;
        for symbol_index in 0..table_size {
            let mut rev_code = 0;
            let code_size = table.code_size[symbol_index];
            if code_size == 0 { continue }

            let mut cur_code = next_code[code_size as usize];
            next_code[code_size as usize] += 1;

            for _ in 0..code_size {
                rev_code = (rev_code << 1) | (cur_code & 1);
                cur_code >>= 1;
            }

            if code_size <= TINFL_FAST_LOOKUP_BITS {
                let k = ((code_size as i16) << 9) | symbol_index as i16;
                while rev_code < TINFL_FAST_LOOKUP_SIZE {
                    table.look_up[rev_code as usize] = k;
                    rev_code += 1 << code_size;
                }
                continue;
            }

            let mut tree_cur = table.look_up[(rev_code & (TINFL_FAST_LOOKUP_SIZE - 1)) as usize];
            if tree_cur == 0 {
                table.look_up[(rev_code & (TINFL_FAST_LOOKUP_SIZE - 1)) as usize] = tree_next as i16;
                tree_cur = tree_next;
                tree_next -= 2;
            }

            rev_code >>= TINFL_FAST_LOOKUP_BITS - 1;
            for _ in TINFL_FAST_LOOKUP_BITS + 1..code_size {
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

        if r.block_type == 0 { break }
        r.block_type -= 1;
    }

    l.counter = 0;
    Action::Jump(DecodeLitlen)
}

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

pub struct LocalVars {
    pub bit_buf: BitBuffer,
    pub num_bits: u32,
    pub dist: u32,
    pub counter: u32,
    pub num_extra: u32,
    pub dist_from_out_buf_start: usize,
}

pub fn decompress_oxide(
    r: &mut tinfl_decompressor,
    in_buf: &[u8],
    out_cur: &mut Cursor<&mut [u8]>,
    flags: u32,
) -> (TINFLStatus, usize, usize) {
    let out_buf_start_pos = out_cur.position() as usize;
    let out_buf_size_mask = if flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0 {
        usize::max_value()
    } else {
        out_cur.get_ref().len() - 1
    };

    // Ensure the output buffer's size is a power of 2, unless the output buffer
    // is large enough to hold the entire output file (in which case it doesn't
    // matter).
    if (out_buf_size_mask.wrapping_add(1) & out_buf_size_mask) != 0 {
        return (TINFLStatus::BadParam, 0, 0)
    }

    let mut in_iter = in_buf.iter();

    let mut state = r.state;//r.state as u8;

    let mut out_buf =
        OutputBuffer::from_slice_and_pos(out_cur.get_mut(), out_buf_start_pos);

    // TODO: Borrow instead of Copy
    let mut l = LocalVars {
        bit_buf: r.bit_buf,
        num_bits: r.num_bits,
        dist: r.dist,
        counter: r.counter,
        num_extra: r.num_extra,
        dist_from_out_buf_start: r.dist_from_out_buf_start,
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
                    r.z_header0 = cmf as u32;
                    Action::Jump(State::ReadZlibFlg)
                })
            }),

            ReadZlibFlg => generate_state!(state, 'state_machine, {
                read_byte(&mut in_iter, flags, |flg| {
                    r.z_header1 = flg as u32;
                    validate_zlib_header(r.z_header0, r.z_header1, flags, out_buf_size_mask)
                })
            }),

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

            BlockTypeNoCompression => generate_state!(state, 'state_machine, {
                pad_to_bytes(&mut l, &mut in_iter, flags, |l| {
                    l.counter = 0;
                    Action::Jump(RawHeader1)
                })
            }),

            RawHeader1 | RawHeader2 => generate_state!(state, 'state_machine, {
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
                    l.counter = r.raw_header[0] as u32 | ((r.raw_header[1] as u32) << 8);
                    let check = (r.raw_header[2] as u16) | ((r.raw_header[3] as u16) << 8);
                    let valid = l.counter == !check as u32;
                    if !valid {
                        Action::Jump(BadRawLength)
                    } else if l.counter == 0 {
                        Action::Jump(BlockDone)
                    } else if l.num_bits != 0 {
                        Action::Jump(RawReadFirstByte)
                    } else {
                        Action::Jump(RawMemcpy1)
                    }
                }
            }),

            RawReadFirstByte => generate_state!(state, 'state_machine, {
                read_bits(&mut l, 8, &mut in_iter, flags, |l, bits| {
                    l.dist = bits as u32;
                    Action::Jump(RawStoreFirstByte)
                })
            }),

            RawStoreFirstByte => generate_state!(state, 'state_machine, {
                if out_buf.bytes_left() == 0 {
                    Action::End(TINFLStatus::HasMoreOutput)
                } else {
                    out_buf.write_byte(l.dist as u8);
                    l.counter -= 1;
                    if l.counter == 0 || l.num_bits == 0 {
                        Action::Jump(RawMemcpy1)
                    } else {
                        Action::Jump(RawStoreFirstByte)
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
                    // Copy as many raw bytes as possible from the input to the output.
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

            ReadTableSizes => generate_state!(state, 'state_machine, {
                if l.counter < 3 {
                    let num_bits = [5, 5, 4][l.counter as usize];
                    read_bits(&mut l, num_bits, &mut in_iter, flags, |l, bits| {
                        r.table_sizes[l.counter as usize] =
                            bits as u32 + MIN_TABLE_SIZES[l.counter as usize] as u32;
                        l.counter += 1;
                        Action::None
                    })
                } else {
                    memset(&mut r.tables[HUFFLEN_TABLE].code_size[..], 0);
                    l.counter = 0;
                    Action::Jump(ReadHufflenTableCodeSize)
                }
            }),

            ReadHufflenTableCodeSize => generate_state!(state, 'state_machine, {
                if l.counter < r.table_sizes[HUFFLEN_TABLE] {
                    read_bits(&mut l, 3, &mut in_iter, flags, |l, bits| {
                        r.tables[HUFFLEN_TABLE].code_size[LENGTH_DEZIGZAG[l.counter as usize] as usize]
                            = bits as u8;
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
                    decode_huffman_code(r, &mut l, HUFFLEN_TABLE, flags, &mut in_iter, |r, l, symbol| {
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
                    })
                } else {
                    if l.counter != r.table_sizes[LITLEN_TABLE] + r.table_sizes[DIST_TABLE] {
                        Action::Jump(BadCodeSizeSum)
                    } else {
                        r.tables[LITLEN_TABLE].code_size[..r.table_sizes[LITLEN_TABLE] as usize]
                            .copy_from_slice(&r.len_codes[..r.table_sizes[LITLEN_TABLE] as usize]);

                        let dist_table_start = r.table_sizes[LITLEN_TABLE] as usize;
                        let dist_table_end = (r.table_sizes[LITLEN_TABLE] + r.table_sizes[DIST_TABLE]) as usize;
                        r.tables[DIST_TABLE].code_size[..r.table_sizes[DIST_TABLE] as usize]
                            .copy_from_slice(&r.len_codes[dist_table_start..dist_table_end]);

                        r.block_type -= 1;
                        init_tree(r, &mut l)
                    }
                }
            }),

            ReadExtraBitsCodeSize => generate_state!(state, 'state_machine, {
                let num_extra = l.num_extra;
                read_bits(&mut l, num_extra, &mut in_iter, flags, |l, mut extra_bits| {
                    // Mask to avoid a bounds check.
                    extra_bits += [3, 3, 11][l.dist as usize - 16 & 3];
                    let val = if l.dist == 16 {
                        r.len_codes[l.counter as usize - 1]
                    } else {
                        0
                    };

                    memset(&mut r.len_codes[l.counter as usize..l.counter as usize + extra_bits as usize], val);
                    l.counter += extra_bits as u32;
                    Action::Jump(ReadLitlenDistTablesCodeSize)
                })
            }),

            DecodeLitlen => generate_state!(state, 'state_machine, {
                if in_iter.len() < 4 || out_buf.bytes_left() < 2 {
                    // See if we can decode a literal with the data we have left.
                    // Jumps to next state (WriteSymbol) if successful.
                    decode_huffman_code(r, &mut l, LITLEN_TABLE, flags, &mut in_iter, |_r, l, symbol| {
                        l.counter = symbol as u32;
                        Action::Jump(WriteSymbol)
                    })
                } else {
                    if cfg!(target_pointer_width = "64") {
                        // Read four bytes into the buffer at once.
                        if l.num_bits < 30 {
                            l.bit_buf |= (read_u32_le(&mut in_iter) as BitBuffer) << l.num_bits;
                            l.num_bits += 32;
                        }
                    } else {
                        // If the buffer is 32-bit wide, read 2 bytes instead.
                        if l.num_bits < 15 {
                            l.bit_buf |= (read_u16_le(&mut in_iter) as BitBuffer) << l.num_bits;
                            l.num_bits += 16;
                        }
                    }

                    let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(l.bit_buf);

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
                            if l.num_bits < 15 {
                                l.bit_buf |= (read_u16_le(&mut in_iter) as BitBuffer) << l.num_bits;
                                l.num_bits += 16;
                            }
                        }

                        let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(l.bit_buf);

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
                    }
                }
            }),

            WriteSymbol => generate_state!(state, 'state_machine, {
                if l.counter >= 256 {
                    Action::Jump(HuffDecodeOuterLoop1)
                } else {
                    if out_buf.bytes_left() > 0 {
                        out_buf.write_byte(l.counter as u8);
                        Action::Jump(DecodeLitlen)
                    } else {
                        Action::End(TINFLStatus::HasMoreOutput)
                    }
                }
            }),

            HuffDecodeOuterLoop1 => generate_state!(state, 'state_machine, {
                // Mask the top bits since they may contain length info.
                l.counter &= 511;

                if l.counter == 256 {
                    // We hit the end of block symbol.
                    Action::Jump(BlockDone)
                } else {
                    // # Optimization
                    // Mask the value to avoid bounds checks
                    // We could use get_unchecked later if can statically verify that
                    // this will never go out of bounds.
                    l.num_extra = LENGTH_EXTRA[(l.counter - 257) as usize & BASE_EXTRA_MASK] as u32;
                    l.counter = LENGTH_BASE[(l.counter - 257) as usize & BASE_EXTRA_MASK] as u32;
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
                    // # Optimization
                    // Mask the value to avoid bounds checks
                    // We could use get_unchecked later if can statically verify that
                    // this will never go out of bounds.
                    l.num_extra = DIST_EXTRA[symbol as usize & BASE_EXTRA_MASK] as u32;
                    l.dist = DIST_BASE[symbol as usize & BASE_EXTRA_MASK] as u32;
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
                // A cursor wrapping a slice can't be larger than usize::max.
                l.dist_from_out_buf_start = out_buf.position();
                if l.dist as usize > l.dist_from_out_buf_start &&
                    (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0) {
                        // We encountered a distance that refers a position before
                        // the start of the decoded data, so we can't continue.
                        Action::Jump(DistanceOutOfBounds)
                } else {
                    let mut source_pos = (l.dist_from_out_buf_start.wrapping_sub(l.dist as usize)) &
                        out_buf_size_mask;

                    let out_len = out_buf.get_ref().len() as usize;
                    let match_end_pos = cmp::max(source_pos, out_buf.position())
                        + l.counter as usize;

                    let mut out_pos = out_buf.position();

                    if match_end_pos > out_len ||
                        // miniz doesn't do this check here. Not sure how it makes sure
                        // that this case doesn't happen.
                        (source_pos >= out_pos && (source_pos - out_pos) < l.counter as usize) {
                        // Not enough space for all of the data in the output buffer,
                        // so copy what we have space for.

                        if l.counter == 0 {
                            Action::Jump(DecodeLitlen)
                        } else {
                            l.counter -= 1;
                            Action::Jump(WriteLenBytesToEnd)
                        }
                    } else {
                        {
                            let out_slice = out_buf.get_mut();
                            let mut match_len = l.counter as usize;
                            // This is faster on x86, but at least on arm, it's slower, so only
                            // use it on x86 for now.
                            if match_len <= l.dist as usize && match_len > 3 &&
                                (cfg!(any(target_arch = "x86", target_arch ="x86_64"))){
                                if source_pos < out_pos {
                                    let (from_slice, to_slice) = out_slice.split_at_mut(out_pos);
                                    to_slice[..match_len]
                                        .copy_from_slice(
                                            &from_slice[source_pos..source_pos + match_len])
                                } else {
                                    let (to_slice, from_slice) = out_slice.split_at_mut(source_pos);
                                    to_slice[out_pos..out_pos + match_len]
                                        .copy_from_slice(&from_slice[..match_len]);
                                }
                                out_pos += match_len;
                            } else {
                                loop {
                                    out_slice[out_pos] = out_slice[source_pos];
                                    out_slice[out_pos + 1] = out_slice[source_pos + 1];
                                    out_slice[out_pos + 2] = out_slice[source_pos + 2];
                                    source_pos += 3;
                                    out_pos += 3;
                                    match_len -= 3;
                                    if match_len < 3 {
                                        break;
                                    }
                                }

                                if match_len > 0 {
                                    out_slice[out_pos] = out_slice[source_pos];
                                    if match_len > 1 {
                                        out_slice[out_pos + 1] = out_slice[source_pos + 1];
                                    }
                                    out_pos += match_len;
                                }
                                l.counter = match_len as u32;
                            }
                        }

                        out_buf.set_position(out_pos);
                        Action::Jump(DecodeLitlen)
                    }
                }
            }),

            WriteLenBytesToEnd => generate_state!(state, 'state_machine, {
                if out_buf.bytes_left() > 0 {
                    let source_pos = l.dist_from_out_buf_start.wrapping_sub(l.dist as usize) & out_buf_size_mask;
                    let val = out_buf.get_ref()[source_pos];
                    l.dist_from_out_buf_start += 1;
                    out_buf.write_byte(val);
                    if l.counter == 0 {
                        Action::Jump(DecodeLitlen)
                    } else {
                        l.counter -= 1;
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

                    l.bit_buf &= (((1 as BitBuffer) << l.num_bits) - 1) as BitBuffer;
                    assert!(l.num_bits == 0);

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
                            r.z_adler32 |= byte as u32;
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
            // BadZlibHeader | BadRawLength | BlockTypeUnexpected | DistanceOutOfBounds |
            // BadTotalSymbols | BadCodeSizeDistPrevLookup | BadCodeSizeSum
            _ => break TINFLStatus::Failed,
        };
    };

    let in_undo = if status != TINFLStatus::NeedsMoreInput &&
        status != TINFLStatus::FailedCannotMakeProgress {
        undo_bytes(&mut l, (in_buf.len() - in_iter.len()) as u32) as usize
    } else { 0 };

    r.state = state.into();
    r.bit_buf = l.bit_buf;
    r.num_bits = l.num_bits;
    r.dist = l.dist;
    r.counter = l.counter;
    r.num_extra = l.num_extra;
    r.dist_from_out_buf_start = l.dist_from_out_buf_start;

    r.bit_buf &= (((1 as BitBuffer) << r.num_bits) - 1) as BitBuffer;

    let need_adler = flags & (TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_COMPUTE_ADLER32) != 0;
    if need_adler && status as i32 >= 0 {
        let out_buf_pos = out_buf.position();
        r.check_adler32 = update_adler32(
            r.check_adler32,
            &out_buf.get_ref()[out_buf_start_pos..out_buf_pos]
        );

        if status == TINFLStatus::Done && flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 && r.check_adler32 != r.z_adler32 {
            status = TINFLStatus::Adler32Mismatch;
        }
    }

    // NOTE: Status here and in miniz_tester doesn't seem to match.
    (
        status,
        in_buf.len() - in_iter.len() - in_undo,
        out_buf.position() - out_buf_start_pos
    )
}


#[cfg(test)]
mod test {
    use super::*;
    //use std::io::Cursor;

    //TODO: Fix these.

    fn tinfl_decompress_oxide<'i>(
        r: &mut tinfl_decompressor,
        input_buffer: &'i [u8],
        output_buffer: &mut [u8],
        flags: u32,
    ) -> (TINFLStatus, &'i [u8], usize) {
        let (status, in_pos, out_pos) = decompress_oxide(r, input_buffer, &mut Cursor::new(output_buffer), flags);
        (status, &input_buffer[in_pos..], out_pos)
    }

    #[test]
    fn decompress_zlib() {
        let encoded =
            [120, 156, 243, 72, 205, 201, 201, 215, 81,
             168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;

        let mut b = tinfl_decompressor::new();
        const LEN: usize = 32;
        let mut b_buf = vec![0; LEN];

        // This should fail with the out buffer being to small.
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);

        assert_eq!(b_status.0, TINFLStatus::Failed);

        let flags = flags | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        b = tinfl_decompressor::new();

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
            let len = text.len().to_le();
            let notlen = !len;
            let mut encoded =
                vec![1, len as u8, (len >> 8) as u8, notlen as u8,
                     (notlen >> 8) as u8];
            encoded.extend_from_slice(&text[..]);
            encoded
        };

        //let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER |
        let flags = TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        let mut b = tinfl_decompressor::new();

        let mut b_buf = vec![0; LEN];

        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);
        assert_eq!(b_buf[..b_status.2], text[..]);
        assert_eq!(b_status.0, TINFLStatus::Done);
    }

    fn masked_lookup(table: &tinfl_huff_table, bit_buf: BitBuffer) -> (i32, u32) {
        let ret = table.lookup(bit_buf);
        (ret.0 & 511, ret.1)
    }

    #[test]
    fn fixed_table_lookup() {
        let mut d = tinfl_decompressor::new();
        d.block_type = 1;
        start_static_table(&mut d);
        let mut l = LocalVars {
            bit_buf: d.bit_buf,
            num_bits: d.num_bits,
            dist: d.dist,
            counter: d.counter,
            num_extra: d.num_extra,
            dist_from_out_buf_start: d.dist_from_out_buf_start,
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
}
