use super::*;

use std::io::Write;
use std::{cmp, slice, ptr};

pub fn memset<T : Clone>(slice: &mut [T], val: T) {
    for x in slice { *x = val.clone() }
}

const START: u32 = 0;
const READ_ZLIB_CMF: u32 = 1;
const READ_ZLIB_FLG: u32 = 2;
const READ_BLOCK_HEADER: u32 = 3;
const BLOCK_TYPE_NO_COMPRESSION: u32 = 5;
const RAW_HEADER1: u32 = 6;
const RAW_HEADER2: u32 = 7;
const RAW_MEMCPY1: u32 = 9;
const BLOCK_TYPE_UNEXPECTED: u32 = 10;
const READ_TABLE_SIZES: u32 = 11;
const READ_HUFFLEN_TABLE_CODE_SIZE: u32 = 14;
const READ_LITLEN_DIST_TABLES_CODE_SIZE: u32 = 16;
const BAD_CODE_SIZE_DIST_PREV_LOOKUP: u32 = 17;
const READ_EXTRA_BITS_CODE_SIZE: u32 = 18;
const BAD_CODE_SIZE_SUM: u32 = 21;
const DECODE_LITLEN: u32 = 23;
const WRITE_SYMBOL: u32 = 24;
const READ_EXTRA_BITS_LITLEN: u32 = 25;
const DECODE_DISTANCE: u32 = 26;
const READ_EXTRA_BITS_DISTANCE: u32 = 27;
const DONE_FOREVER: u32 = 34;
const BAD_TOTAL_SYMBOLS: u32 = 35;
const BAD_ZLIB_HEADER: u32 = 36;
const DISTANCE_OUT_OF_BOUNDS: u32 = 37;
const RAW_MEMCPY2: u32 = 38;
const BAD_RAW_LENGTH: u32 = 39;
const READ_ADLER32: u32 = 41;
const RAW_READ_FIRST_BYTE: u32 = 51;
const RAW_STORE_FIRST_BYTE: u32 = 52;
const WRITE_LEN_BYTES_TO_END: u32 = 53;

// Not in miniz - corresponds to main loop end there.
const BLOCK_DONE: u32 = 100;

const HUFF_DECODE_LOOP_START: u32 = 105;
const HUFF_DECODE_OUTER_LOOP1: u32 = 101;
const HUFF_DECODE_OUTER_LOOP2: u32 = 102;

// Not sure why miniz uses 32-bit values for these, maybe alignment/cache again?
const LENGTH_BASE: [i32; 31] = [
    3,  4,  5,  6,  7,  8,  9,  10,  11,  13,  15,  17,  19,  23, 27, 31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 0,  0
];

const LENGTH_EXTRA: [i32; 31] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1,
    1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4,
    4, 4, 5, 5, 5, 5, 0, 0, 0
];

const DIST_BASE: [i32; 32] = [
    1,    2,    3,    4,    5,    7,     9,     13,    17,  25,   33,
    49,   65,   97,   129,  193,  257,   385,   513,   769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 0,   0
];

const DIST_EXTRA: [i32; 32] = [
    0, 0, 0,  0,  1,  1,  2,  2,  3,  3,
    4, 4, 5,  5,  6,  6,  7,  7,  8,  8,
    9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
    13, 13
];

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
fn transfer_unaligned_u64(buf: &mut &mut[u8], from: isize, to: isize) {
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
    assert!(0b000100000 == 32);
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
        Action::Jump(BAD_ZLIB_HEADER)
    } else {
        Action::Next
    }
}


enum Action {
    None,
    Next,
    Jump(u32),
    End(TINFLStatus),
}

/// Try to decode the next huffman code, and puts it in the counter field of the decompressor
/// if successful.
///
/// # Returns
/// Action::Next on success, Action::End if there are not enough data left to decode a symbol.
fn decode_huffman_code<F>(
    r: &mut tinfl_decompressor,
    table: usize,
    flags: u32,
    in_iter: &mut slice::Iter<u8>,
    f: F,
) -> Action
    where F: FnOnce(&mut tinfl_decompressor, i32) -> Action,
{
    // As the huffman codes can be up to 15 bits long we need at least 15 bits
    // ready in the bit buffer to start decoding the next huffman code.
    if r.num_bits < 15 {
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
                let mut temp = r.tables[table].fast_lookup(r.bit_buf) as i32;

                if temp >= 0 {
                    let code_len = (temp >> 9) as u32;
                    if (code_len != 0) && (r.num_bits >= code_len) {
                        break;
                    }
                } else if r.num_bits > TINFL_FAST_LOOKUP_BITS.into() {
                    let mut code_len = TINFL_FAST_LOOKUP_BITS as u32;
                    loop {
                        temp = r.tables[table].tree[
                            (!temp + ((r.bit_buf >> code_len) & 1) as i32) as usize] as i32;
                        code_len += 1;
                        if temp >= 0 || r.num_bits < code_len + 1 {
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
                r.bit_buf |= (byte as BitBuffer) << r.num_bits;
                r.num_bits += 8;

                if r.num_bits >= 15 {
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

            r.bit_buf |= (b0 << r.num_bits) | (b1 << r.num_bits + 8);
            r.num_bits += 16;
        }
    }

    // We now have at least 15 bits in the input buffer.
    let mut symbol = r.tables[table].fast_lookup(r.bit_buf) as i32;
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
        let res = r.tables[table].tree_lookup(symbol, r.bit_buf, TINFL_FAST_LOOKUP_BITS as u32);
        symbol = res.0;
        code_len = res.1 as u32;
    };

    r.bit_buf >>= code_len as u32;
    r.num_bits -= code_len;
    f(r, symbol)
}

#[inline]
fn bytes_left(out_buf: &Cursor<&mut [u8]>) -> usize {
    out_buf.get_ref().len() - out_buf.position() as usize
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

/// Write one byte to the cursor.
///
/// This is intended for cases where we've already checked that there is space left.
///
/// # Panics
/// Panics if the cursor is full.
#[inline]
fn write_byte(out_buf: &mut Cursor<&mut [u8]>, byte: u8) {
    out_buf.write_all(&[byte]).expect("Bug! Out buffer unexpectedly full!");
}

#[inline]
fn read_bits<F>(
    r: &mut tinfl_decompressor,
    amount: u32,
    in_iter: &mut slice::Iter<u8>,
    flags: u32,
    f: F,
) -> Action
    where F: FnOnce(&mut tinfl_decompressor, BitBuffer) -> Action,
{
    while r.num_bits < amount {
        match read_byte(in_iter, flags, |byte| {
            r.bit_buf |= (byte as BitBuffer) << r.num_bits;
            r.num_bits += 8;
            Action::Next
        }) {
            Action::Next => (),
            action => return action,
        }
    }

    let bits = r.bit_buf & ((1 << amount) - 1);
    r.bit_buf >>= amount;
    r.num_bits -= amount;
    f(r, bits)
}

#[inline]
fn pad_to_bytes<F>(
    r: &mut tinfl_decompressor,
    in_iter: &mut slice::Iter<u8>,
    flags: u32,
    f: F,
) -> Action
    where F: FnOnce(&mut tinfl_decompressor) -> Action,
{
    let num_bits = r.num_bits & 7;
    read_bits(r, num_bits, in_iter, flags, |r, _| f(r))
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
fn undo_bytes(r: &mut tinfl_decompressor, max: u32) -> u32 {
    let res = cmp::min((r.num_bits >> 3), max);
    r.num_bits -= res << 3;
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

fn init_tree(r: &mut tinfl_decompressor) -> Action {
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
            return Action::Jump(BAD_TOTAL_SYMBOLS);
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
            r.counter = 0;
            return Action::Jump(READ_LITLEN_DIST_TABLES_CODE_SIZE);
        }

        if r.block_type == 0 { break }
        r.block_type -= 1;
    }

    r.counter = 0;
    Action::Jump(DECODE_LITLEN)
}

pub fn decompress_oxide(
    r: &mut tinfl_decompressor,
    in_buf: &[u8],
    out_buf: &mut Cursor<&mut [u8]>,
    flags: u32,
) -> (TINFLStatus, usize, usize) {
    let out_buf_start_pos = out_buf.position() as usize;
    let out_buf_size_mask = if flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0 {
        usize::max_value()
    } else {
        out_buf.get_ref().len() - 1
    };

    // Ensure the output buffer's size is a power of 2, unless the output buffer
    // is large enough to hold the entire output file (in which case it doesn't
    // matter).
    if (out_buf_size_mask.wrapping_add(1) & out_buf_size_mask) != 0 {
        return (TINFLStatus::BadParam, 0, 0)
    }

    let mut in_iter = in_buf.iter();
    let mut status = loop {
        let action = match r.state {
            START => {
                r.bit_buf = 0;
                r.num_bits = 0;
                r.dist = 0;
                r.counter = 0;
                r.num_extra = 0;
                r.z_header0 = 0;
                r.z_header1 = 0;
                r.z_adler32 = 1;
                r.check_adler32 = 1;
                if flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                    Action::Next
                } else {
                    Action::Jump(READ_BLOCK_HEADER)
                }
            },

            READ_ZLIB_CMF => read_byte(&mut in_iter, flags, |cmf| {
                r.z_header0 = cmf as u32;
                Action::Next
            }),

            READ_ZLIB_FLG => read_byte(&mut in_iter, flags, |flg| {
                r.z_header1 = flg as u32;
                validate_zlib_header(r.z_header0, r.z_header1, flags, out_buf_size_mask)
            }),

            READ_BLOCK_HEADER => {
                read_bits(r, 3, &mut in_iter, flags, |r, bits| {
                    r.finish = (bits & 1) as u32;
                    r.block_type = (bits >> 1) as u32;
                    match r.block_type {
                        0 => Action::Jump(BLOCK_TYPE_NO_COMPRESSION),
                        1 => {
                            start_static_table(r);
                            init_tree(r)
                        },
                        2 => {
                            r.counter = 0;
                            Action::Jump(READ_TABLE_SIZES)
                        },
                        3 => Action::Jump(BLOCK_TYPE_UNEXPECTED),
                        _ => panic!("Value greater than 3 stored in 2 bits"),
                    }
                })
            },

            BLOCK_TYPE_NO_COMPRESSION => pad_to_bytes(r, &mut in_iter, flags, |r| {
                r.counter = 0;
                Action::Next
            }),

            RAW_HEADER1 | RAW_HEADER2 => {
                if r.counter < 4 {
                    // Read block length and block length check.
                    if r.num_bits != 0 {
                        read_bits(r, 8, &mut in_iter, flags, |r, bits| {
                            r.raw_header[r.counter as usize] = bits as u8;
                            r.counter += 1;
                            Action::None
                        })
                    } else {
                        read_byte(&mut in_iter, flags, |byte| {
                            r.raw_header[r.counter as usize] = byte;
                            r.counter += 1;
                            Action::None
                        })
                    }
                } else {
                    // Check if the length value of a raw block is correct.
                    // The 2 first (2-byte) words in a raw header are the length and the
                    // ones complement of the length.
                    r.counter = r.raw_header[0] as u32 | ((r.raw_header[1] as u32) << 8);
                    let check = (r.raw_header[2] as u16) | ((r.raw_header[3] as u16) << 8);
                    let valid = r.counter == !check as u32;
                    if !valid {
                        Action::Jump(BAD_RAW_LENGTH)
                    } else if r.counter == 0 {
                        Action::Jump(BLOCK_DONE)
                    } else if r.num_bits != 0 {
                        Action::Jump(RAW_READ_FIRST_BYTE)
                    } else {
                        Action::Jump(RAW_MEMCPY1)
                    }
                }
            }

            RAW_READ_FIRST_BYTE => {
                read_bits(r, 8, &mut in_iter, flags, |r, bits| {
                    r.dist = bits as u32;
                    Action::Jump(RAW_STORE_FIRST_BYTE)
                })
            },

            RAW_STORE_FIRST_BYTE => {
                match out_buf.write_all(&[r.dist as u8]) {
                    Ok(_) => {
                        r.counter -= 1;
                        if r.counter == 0 || r.num_bits == 0 {
                            Action::Jump(RAW_MEMCPY1)
                        } else {
                            Action::Jump(RAW_STORE_FIRST_BYTE)
                        }
                    },
                    Err(_) => Action::End(TINFLStatus::HasMoreOutput),
                }
            }

            RAW_MEMCPY1 => {
                if r.counter == 0 {
                    Action::Jump(BLOCK_DONE)
                } else if out_buf.position() as usize == out_buf.get_ref().len() {
                    Action::End(TINFLStatus::HasMoreOutput)
                } else {
                    Action::Jump(RAW_MEMCPY2)
                }
            }

            RAW_MEMCPY2 => {
                if in_iter.len() > 0 {
                    // Copy as many raw bytes as possible from the input to the output.
                    // Raw block lengths are limited to 64 * 1024, so casting through usize and u32
                    // is not an issue.
                    let space_left = bytes_left(out_buf);
                    let bytes_to_copy = cmp::min(cmp::min(
                        space_left,
                        in_iter.len()),
                        r.counter as usize
                    );

                    out_buf.write(&in_iter.as_slice()[..bytes_to_copy])
                        .expect("Bug! Write fail!");

                    (&mut in_iter).nth(bytes_to_copy - 1);
                    r.counter -= bytes_to_copy as u32;
                    Action::Jump(RAW_MEMCPY1)
                } else {
                    end_of_input(flags)
                }
            }

            READ_TABLE_SIZES => {
                if r.counter < 3 {
                    let num_bits = [5, 5, 4][r.counter as usize];
                    read_bits(r, num_bits, &mut in_iter, flags, |r, bits| {
                        r.table_sizes[r.counter as usize] = bits as u32 + MIN_TABLE_SIZES[r.counter as usize];
                        r.counter += 1;
                        Action::None
                    })
                } else {
                    memset(&mut r.tables[HUFFLEN_TABLE].code_size[..], 0);
                    r.counter = 0;
                    Action::Jump(READ_HUFFLEN_TABLE_CODE_SIZE)
                }
            },

            READ_HUFFLEN_TABLE_CODE_SIZE => {
                if r.counter < r.table_sizes[HUFFLEN_TABLE] {
                    read_bits(r, 3, &mut in_iter, flags, |r, bits| {
                        r.tables[HUFFLEN_TABLE].code_size[LENGTH_DEZIGZAG[r.counter as usize] as usize]
                            = bits as u8;
                        r.counter += 1;
                        Action::None
                    })
                } else {
                    r.table_sizes[HUFFLEN_TABLE] = 19;
                    init_tree(r)
                }
            },

            READ_LITLEN_DIST_TABLES_CODE_SIZE => {
                if r.counter < r.table_sizes[LITLEN_TABLE] + r.table_sizes[DIST_TABLE] {
                    decode_huffman_code(r, HUFFLEN_TABLE, flags, &mut in_iter, |r, symbol| {
                        r.dist = symbol as u32;
                        if r.dist < 16 {
                            r.len_codes[r.counter as usize] = r.dist as u8;
                            r.counter += 1;
                            Action::None
                        } else if r.dist == 16 && r.counter == 0 {
                            Action::Jump(BAD_CODE_SIZE_DIST_PREV_LOOKUP)
                        } else {
                            r.num_extra = [2, 3, 7][r.dist as usize - 16];
                            Action::Jump(READ_EXTRA_BITS_CODE_SIZE)
                        }
                    })
                } else {
                    if r.counter != r.table_sizes[LITLEN_TABLE] + r.table_sizes[DIST_TABLE] {
                        Action::Jump(BAD_CODE_SIZE_SUM)
                    } else {
                        r.tables[LITLEN_TABLE].code_size[..r.table_sizes[LITLEN_TABLE] as usize]
                            .copy_from_slice(&r.len_codes[..r.table_sizes[LITLEN_TABLE] as usize]);

                        let dist_table_start = r.table_sizes[LITLEN_TABLE] as usize;
                        let dist_table_end = (r.table_sizes[LITLEN_TABLE] + r.table_sizes[DIST_TABLE]) as usize;
                        r.tables[DIST_TABLE].code_size[..r.table_sizes[DIST_TABLE] as usize]
                            .copy_from_slice(&r.len_codes[dist_table_start..dist_table_end]);

                        r.block_type -= 1;
                        init_tree(r)
                    }
                }
            },

            READ_EXTRA_BITS_CODE_SIZE => {
                let num_extra = r.num_extra;
                read_bits(r, num_extra, &mut in_iter, flags, |r, mut extra_bits| {
                    extra_bits += [3, 3, 11][r.dist as usize - 16];
                    let val = if r.dist == 16 {
                        r.len_codes[r.counter as usize - 1]
                    } else {
                        0
                    };

                    memset(&mut r.len_codes[r.counter as usize..r.counter as usize + extra_bits as usize], val);
                    r.counter += extra_bits as u32;
                    Action::Jump(READ_LITLEN_DIST_TABLES_CODE_SIZE)
                })
            },

            DECODE_LITLEN => {
                if in_iter.len() < 4 || bytes_left(out_buf) < 2 {
                    // See if we can decode a literal with the data we have left.
                    // Jumps to next state (WRITE_SYMBOL) if successful.
                    decode_huffman_code(r, LITLEN_TABLE, flags, &mut in_iter, |r, symbol| {
                        r.counter = symbol as u32;
                        Action::Next
                    })
                } else {
                    if cfg!(target_pointer_width = "64") {
                        // Read four bytes into the buffer at once.
                        if r.num_bits < 30 {
                            r.bit_buf |= (read_u32_le(&mut in_iter) as BitBuffer) << r.num_bits;
                            r.num_bits += 32;
                        }
                    } else {
                        // If the buffer is 32-bit wide, read 2 bytes instead.
                        if r.num_bits < 15 {
                            r.bit_buf |= (read_u16_le(&mut in_iter) as BitBuffer) << r.num_bits;
                            r.num_bits += 16;
                        }
                    }

                    let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(r.bit_buf);

                    r.counter = symbol as u32;
                    r.bit_buf >>= code_len;
                    r.num_bits -= code_len;

                    if (r.counter & 256) != 0 {
                        // The symbol is not a literal.
                        Action::Jump(HUFF_DECODE_OUTER_LOOP1)
                    } else {
                        // If we have a 32-bit buffer we need to read another two bytes now
                        // to have enough bits to keep going.
                        if cfg!(not(target_pointer_width = "64")) {
                            if r.num_bits < 15 {
                                r.bit_buf |= (read_u16_le(&mut in_iter) as BitBuffer) << r.num_bits;
                                r.num_bits += 16;
                            }
                        }

                        let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(r.bit_buf);

                        r.bit_buf >>= code_len;
                        r.num_bits -= code_len;
                        // The previous symbol was a literal, so write it directly and check
                        // the next one.
                        write_byte(out_buf, r.counter as u8);
                        if (symbol & 256) != 0 {
                            r.counter = symbol as u32;
                            // The symbol is a length value.
                            Action::Jump(HUFF_DECODE_OUTER_LOOP1)
                        } else {
                            // The symbol is a literal, so write it directly and continue.
                            write_byte(out_buf, symbol as u8);
                            Action::None
                        }
                    }
                }
            },

            WRITE_SYMBOL => {
                if r.counter >= 256 {
                    Action::Jump(HUFF_DECODE_OUTER_LOOP1)
                } else {
                    if bytes_left(out_buf) > 0 {
                        write_byte(out_buf, r.counter as u8);
                        Action::Jump(DECODE_LITLEN)
                    } else {
                        Action::End(TINFLStatus::HasMoreOutput)
                    }
                }
            },

            HUFF_DECODE_OUTER_LOOP1 => {
                // Mask the top bits since they may contain length info.
                r.counter &= 511;

                if r.counter == 256 {
                    // We hit the end of block symbol.
                    Action::Jump(BLOCK_DONE)
                } else {
                    r.num_extra = LENGTH_EXTRA[(r.counter - 257) as usize] as u32;
                    r.counter = LENGTH_BASE[(r.counter - 257) as usize] as u32;
                    // Length and distance codes have a number of extra bits depending on
                    // the base, which together with the base gives us the exact value.
                    if r.num_extra != 0 {
                        Action::Jump(READ_EXTRA_BITS_LITLEN)
                    } else {
                        Action::Jump(DECODE_DISTANCE)
                    }
                }
            },

            READ_EXTRA_BITS_LITLEN => {
                let num_extra = r.num_extra;
                read_bits(r, num_extra, &mut in_iter, flags, |r, extra_bits| {
                    r.counter += extra_bits as u32;
                    Action::Jump(DECODE_DISTANCE)
                })
            },

            DECODE_DISTANCE => {
                decode_huffman_code(r, DIST_TABLE, flags, &mut in_iter, |r, symbol| {
                    r.dist = symbol as u32;
                    r.num_extra = DIST_EXTRA[r.dist as usize] as u32;
                    r.dist = DIST_BASE[r.dist as usize] as u32;
                    if r.num_extra != 0 {
                        // READ_EXTRA_BITS_DISTACNE
                        Action::Next
                    } else {
                        Action::Jump(HUFF_DECODE_OUTER_LOOP2)
                    }
                })
            },

            READ_EXTRA_BITS_DISTANCE => {
                let num_extra = r.num_extra;
                read_bits(r, num_extra, &mut in_iter, flags, |r, extra_bits| {
                    r.dist += extra_bits as u32;
                    Action::Jump(HUFF_DECODE_OUTER_LOOP2)
                })
            },

            HUFF_DECODE_OUTER_LOOP2 => {
                // A cursor wrapping a slice can't be larger than usize::max.
                r.dist_from_out_buf_start = out_buf.position() as usize;
                if r.dist as usize > r.dist_from_out_buf_start &&
                    (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0) {
                        // We encountered a distance that refers a position before
                        // the start of the decoded data, so we can't continue.
                        Action::Jump(DISTANCE_OUT_OF_BOUNDS)
                } else {
                    let mut source_pos = (r.dist_from_out_buf_start.wrapping_sub(r.dist as usize)) &
                        out_buf_size_mask;

                    let out_len = out_buf.get_ref().len() as usize;
                    let match_end_pos = cmp::max(source_pos, out_buf.position() as usize)
                        + r.counter as usize;



                    let mut out_pos = out_buf.position() as usize;

                    if match_end_pos > out_len ||
                        // miniz doesn't do this check here. Not sure how it makes sure
                        // that this case doesn't happen.
                        (source_pos >= out_pos && (source_pos - out_pos) < r.counter as usize) {
                        // Not enough space for all of the data in the output buffer,
                        // so copy what we have space for.

                        if r.counter == 0 {
                            Action::Jump(DECODE_LITLEN)
                        } else {
                            r.counter -= 1;
                            Action::Jump(WRITE_LEN_BYTES_TO_END)
                        }
                    } else {
                        {
                            let out_slice = out_buf.get_mut();
                            let match_len = r.counter as usize;
                            if r.counter <= r.dist {
                                if source_pos < out_pos {
                                    let (from_slice, to_slice) = out_slice.split_at_mut(out_pos);
                                    to_slice[..match_len].copy_from_slice(
                                        &from_slice[source_pos..source_pos + match_len]
                                    );
                                } else {
                                    let (to_slice, from_slice) = out_slice.split_at_mut(source_pos);
                                    to_slice[out_pos..out_pos + match_len].copy_from_slice(
                                        &from_slice[..match_len]
                                    );
                                }
                                out_pos += match_len;
                            } else {
                                while r.counter >= 3 {
                                    out_slice[out_pos] = out_slice[source_pos];
                                    out_slice[out_pos + 1] = out_slice[source_pos + 1];
                                    out_slice[out_pos + 2] = out_slice[source_pos + 2];
                                    source_pos += 3;
                                    out_pos += 3;
                                    r.counter -= 3;
                                }

                                if r.counter > 0 {
                                    out_slice[out_pos] = out_slice[source_pos];
                                    if r.counter > 1 {
                                        out_slice[out_pos + 1] = out_slice[source_pos + 1];
                                    }
                                    out_pos += r.counter as usize;
                                }
                            }


                        }
                        out_buf.set_position(out_pos as u64);
                        Action::Jump(DECODE_LITLEN)
                    }
                }
            },

            WRITE_LEN_BYTES_TO_END => {
                if bytes_left(out_buf) > 0 {
                    let source_pos = r.dist_from_out_buf_start.wrapping_sub(r.dist as usize) & out_buf_size_mask;
                    let val = out_buf.get_ref()[source_pos];
                    r.dist_from_out_buf_start += 1;
                    write_byte(out_buf, val);
                    if r.counter == 0 {
                        Action::Jump(DECODE_LITLEN)
                    } else {
                        r.counter -= 1;
                        Action::None
                    }
                } else {
                    Action::End(TINFLStatus::HasMoreOutput)
                }
            },

            BAD_ZLIB_HEADER | BAD_RAW_LENGTH | BLOCK_TYPE_UNEXPECTED | DISTANCE_OUT_OF_BOUNDS |
            BAD_TOTAL_SYMBOLS | BAD_CODE_SIZE_DIST_PREV_LOOKUP | BAD_CODE_SIZE_SUM
                => Action::End(TINFLStatus::Failed),

            DONE_FOREVER => Action::End(TINFLStatus::Done),

            BLOCK_DONE => {
                // End once we've read the last block.
                if r.finish != 0 {
                    pad_to_bytes(r, &mut in_iter, flags, |_| Action::None);

                    let in_consumed = in_buf.len() - in_iter.len();
                    let undo = undo_bytes(r, in_consumed as u32) as usize;
                    in_iter = in_buf[in_consumed - undo..].iter();

                    r.bit_buf &= ((1u64 << r.num_bits) - 1) as BitBuffer;
                    assert!(r.num_bits == 0);

                    if flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                        r.counter = 0;
                        Action::Jump(READ_ADLER32)
                    } else {
                        Action::Jump(DONE_FOREVER)
                    }
                } else {
                    Action::Jump(READ_BLOCK_HEADER)
                }
            },

            READ_ADLER32 => {
                if r.counter < 4 {
                    if r.num_bits != 0 {
                        read_bits(r, 8, &mut in_iter, flags, |r, bits| {
                            r.z_adler32 <<= 8;
                            r.z_adler32 |= bits as u32;
                            r.counter += 1;
                            Action::None
                        })
                    } else {
                        read_byte(&mut in_iter, flags, |byte| {
                            r.z_adler32 <<= 8;
                            r.z_adler32 |= byte as u32;
                            r.counter += 1;
                            Action::None
                        })
                    }
                } else {
                    Action::Jump(DONE_FOREVER)
                }
            },

            _ => panic!("Unknown state"),
        };

        match action {
            Action::None => (),
            Action::Next => r.state += 1,
            Action::Jump(state) => r.state = state,
            Action::End(status) => break status,
        }
    };

    let in_undo = if status != TINFLStatus::NeedsMoreInput && status != TINFLStatus::FailedCannotMakeProgress {
        undo_bytes(r, (in_buf.len() - in_iter.len()) as u32) as usize
    } else { 0 };

    r.bit_buf &= ((1u64 << r.num_bits) - 1) as BitBuffer;

    let need_adler = flags & (TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_COMPUTE_ADLER32) != 0;
    if need_adler && status as i32 >= 0 {
        r.check_adler32 = ::mz_adler32_oxide(
            r.check_adler32,
            &out_buf.get_ref()[out_buf_start_pos..out_buf.position() as usize]
        );

        if status == TINFLStatus::Done && flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 && r.check_adler32 != r.z_adler32 {
            status = TINFLStatus::Adler32Mismatch;
        }
    }

    // NOTE: Status here and in miniz_tester doesn't seem to match.
    (
        status,
        in_buf.len() - in_iter.len() - in_undo,
        out_buf.position() as usize - out_buf_start_pos
    )
}


#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[cfg(feature = "build_non_rust")]
    fn tinfl_decompress_miniz<'i>(
        r: &mut tinfl_decompressor,
        input_buffer: &'i [u8],
        output_buffer: &mut [u8],
        flags: u32,
    ) -> (TINFLStatus, &'i [u8], usize) {
        unsafe {
            let r = r as *mut tinfl_decompressor;
            let mut in_buf_size = input_buffer.len();
            let in_buf_next = input_buffer.as_ptr();
            let mut out_buf_size = output_buffer.len();
            let out_buf_start = output_buffer.as_mut_ptr();
            let out_buf_next = out_buf_start;
            // Confusingly, in_buf_size/out_buf_size
            // gets set to the number of bytes read from the input/output buffers.
            let status =
                tinfl_decompress(r, in_buf_next, &mut in_buf_size, out_buf_start, out_buf_next,
                                 &mut out_buf_size, flags);
            let istatus = status as i32;
            assert!(istatus >= TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS &&
                    istatus <= TINFL_STATUS_HAS_MORE_OUTPUT,
                    "Invalid status code {}!", istatus);

            let (remaining_start, out_start) = if status == TINFLStatus::BadParam {
                (0, 0)
            } else {
                (in_buf_size, out_buf_size)
            };

            (status, &input_buffer[remaining_start..], out_start)
        }
    }

    fn tinfl_decompress_oxide<'i>(
        r: &mut tinfl_decompressor,
        input_buffer: &'i [u8],
        output_buffer: &mut [u8],
        flags: u32,
    ) -> (TINFLStatus, &'i [u8], usize) {
        let (status, in_pos, out_pos) = decompress_oxide(r, input_buffer, &mut Cursor::new(output_buffer), flags);
        (status, &input_buffer[in_pos..], out_pos)
    }

    /// Dummy redirect
    #[cfg(not(feature = "build_non_rust"))]
    use self::tinfl_decompress_oxide as tinfl_decompress_miniz;

    #[test]
    fn decompress_zlib() {
        let encoded =
            [120, 156, 243, 72, 205, 201, 201, 215, 81,
             168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;

        let mut a = tinfl_decompressor::new();
        let mut b = tinfl_decompressor::new();
        const LEN: usize = 32;
        let mut a_buf = vec![0; LEN];
        let mut b_buf = vec![0; LEN];

        // These should fail with the out buffer being to small.
        let a_status = tinfl_decompress_miniz(&mut a, &encoded[..], a_buf.as_mut_slice(), flags);
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);

        assert_eq!(a_status, b_status);
        assert_eq!(a_buf.as_slice(), b_buf.as_slice());
        assert_eq!(a.z_header0, b.z_header0);
        assert_eq!(a.state, b.state);

        let flags = flags | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        a = tinfl_decompressor::new();
        b = tinfl_decompressor::new();

        // With TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF set this should no longer fail.
        let a_status = tinfl_decompress_miniz(&mut a, &encoded[..], a_buf.as_mut_slice(), flags);
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);

        assert_eq!(a_buf[..a_status.2], b"Hello, zlib!"[..]);
        assert_eq!(b_buf[..b_status.2], b"Hello, zlib!"[..]);
        assert_eq!(a_status, b_status);
        assert_eq!(a_buf.as_slice(), b_buf.as_slice());
        assert_eq!(a.z_header0, b.z_header0);
        assert_eq!(a.state, b.state);
        // TODO: Fully check that a and b are equal.
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

        let mut a = tinfl_decompressor::new();
        let mut b = tinfl_decompressor::new();

        let mut a_buf = vec![0; LEN];
        let mut b_buf = vec![0; LEN];

        let a_status = tinfl_decompress_miniz(&mut a, &encoded[..], a_buf.as_mut_slice(), flags);
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], b_buf.as_mut_slice(), flags);
        assert_eq!(a_status, b_status);
        assert_eq!(b_buf[..b_status.2], text[..]);
        assert_eq!(a_status.0, TINFLStatus::Done);
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
        init_tree(&mut d);
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
