use super::*;

use std::io::Write;
use std::cmp;

pub fn memset<T : Clone>(slice: &mut [T], val: T) {
    for x in slice { *x = val.clone() }
}

// Not in miniz
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
const BAD_TOTAL_SYMBOLS: u32 = 35;
const BAD_ZLIB_HEADER: u32 = 36;
const RAW_MEMCPY2: u32 = 38;
const BAD_RAW_LENGTH: u32 = 39;
const RAW_READ_FIRST_BYTE: u32 = 51;
const RAW_STORE_FIRST_BYTE: u32 = 52;

// Not in miniz - corresponds to main loop end there.
const BLOCK_DONE: u32 = 100;

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
    Next,
    Jump(u32),
    End(TINFLStatus),
}

#[inline]
fn read_byte<'a, Iter, F>(in_iter: &mut Iter, flags: u32, f: F) -> Action
    where Iter: Iterator<Item=&'a u8>,
          F: FnOnce(u8) -> Action,
{
    match in_iter.next() {
        None => end_of_input(flags),
        Some(&byte) => {
            f(byte)
        }
    }
}

#[inline]
fn read_bits<'a, Iter, F>(
    r: &mut tinfl_decompressor,
    amount: u32,
    in_iter: &mut Iter,
    flags: u32,
    f: F,
) -> Action
    where Iter: Iterator<Item=&'a u8>,
          F: FnOnce(&mut tinfl_decompressor, BitBuffer) -> Action,
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
fn end_of_input(flags: u32) -> Action {
    Action::End(if flags & TINFL_FLAG_HAS_MORE_INPUT != 0 {
        TINFLStatus::NeedsMoreInput
    } else {
        TINFLStatus::FailedCannotMakeProgress
    })
}

fn start_static_table(r: &mut tinfl_decompressor) {
    r.table_sizes[0] = 288;
    r.table_sizes[1] = 32;
    memset(&mut r.tables[1].code_size[..32], 5);
    memset(&mut r.tables[0].code_size[0..144], 8);
    memset(&mut r.tables[0].code_size[144..256], 9);
    memset(&mut r.tables[0].code_size[256..280], 7);
    memset(&mut r.tables[0].code_size[280..288], 8);
}

fn init_tree(r: &mut tinfl_decompressor) -> Action {
    for table_num in (0..r.block_type + 1).rev() {
        let table = &mut r.tables[table_num as usize];
        let table_size = r.table_sizes[table_num as usize] as usize;
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

        if table_num == 2 {
            r.counter = 0;
            return Action::Jump(16);
        }
    }

    r.counter = 0;
    Action::Jump(23)
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

            BLOCK_TYPE_NO_COMPRESSION => {
                // Skip the remaining bits up to the byte boundary.
                let num_bits = r.num_bits & 7;
                read_bits(r, num_bits, &mut in_iter, flags ,|r,_| {
                    // Reset counter for the next state.
                    r.counter = 0;
                    Action::Next
                })
            }

            RAW_HEADER1 | RAW_HEADER2 => {
                if r.counter < 4 {
                    // Read block length and block length check.
                    let ret = if r.num_bits != 0 {
                        read_bits(r, 8, &mut in_iter, flags, |r, bits| {
                            r.raw_header[r.counter as usize] = bits as u8;
                            Action::Jump(RAW_HEADER1)
                        })
                    } else {
                        read_byte(&mut in_iter, flags, |byte| {
                            r.raw_header[r.counter as usize] = byte;
                            Action::Jump(RAW_HEADER2)
                        })
                    };
                    r.counter += 1;
                    ret
                } else {
                    // Check if the length value of a raw block is correct.
                    // The 2 first (2-byte) words in a raw header are the length and the
                    // ones complement of the length.
                    r.counter = r.raw_header[0] as u32 | ((r.raw_header[1] as u32) << 8);
                    let check = (r.raw_header[2] as u16) | ((r.raw_header[3] as u16) << 8);
                    let valid = r.counter == !check as u32;
                    if !valid {
                        Action::Jump(BAD_RAW_LENGTH)
                    } else if r.num_bits != 0 {
                        Action::Jump(RAW_READ_FIRST_BYTE)
                    } else {
                        Action::Jump(RAW_MEMCPY1)
                    }
                }
            }

            RAW_READ_FIRST_BYTE => {
                read_bits(r, 8, &mut in_iter, flags, |r, bits| {
                    r.dist = bits;
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
                    Err(_) => {
                        Action::End(TINFLStatus::HasMoreOutput)
                    }
                }
            }

            RAW_MEMCPY1 => {
                if out_buf.position() as usize == out_buf.get_ref().len() {
                    Action::End(TINFLStatus::HasMoreOutput)
                } else if r.counter > 0 {
                    Action::Jump(RAW_MEMCPY2)
                } else {
                    Action::Jump(BLOCK_DONE)
                }
            }

            RAW_MEMCPY2 => {
                if in_iter.len() > 0 {
                    // Copy as many raw bytes as possible from the input to the output.
                    // Raw block lengths are limited to 64 * 1024, so casting through usize and u32
                    // is not an issue.
                    let space_left = out_buf.get_ref().len() - (out_buf.position() as usize);
                    let bytes_to_copy = cmp::min(cmp::min(in_iter.len(), space_left),
                                                 r.counter as usize);
                    out_buf.write(&in_iter.as_slice()[..bytes_to_copy])
                        .expect("Bug! Write fail!");
                    (&mut in_iter).nth(bytes_to_copy);
                    r.counter -= bytes_to_copy as u32;
                    Action::Jump(RAW_MEMCPY1)
                } else {
                    end_of_input(flags)
                }
            }

            BAD_ZLIB_HEADER | BAD_RAW_LENGTH | BLOCK_TYPE_UNEXPECTED
                => Action::End(TINFLStatus::Failed),

            BLOCK_DONE =>
                // End once we've read the last block.
                if r.finish != 0 {
                    //TODO: Do the ending bit (after the while loop
                    // before common_exit l 562)
                    // which includes reading and storeing adler32 from the end, so
                    // we don't fail on adler32 mismatch here.)
                    Action::End(TINFLStatus::Done)
                } else {
                    Action::Jump(READ_BLOCK_HEADER)
                },

            _ => unsafe {
                let mut in_len = in_iter.len();
                let out_pos = out_buf.position() as usize;
                let mut out_len = out_buf.get_ref().len() - out_pos;
                let out_buf = out_buf.get_mut();
                let status = tinfl_decompress(
                    r,
                    in_iter.as_slice().as_ptr(),
                    &mut in_len,
                    // as_mut_ptr to process zero sized slices
                    (*out_buf).as_mut_ptr(),
                    (*out_buf).as_mut_ptr().offset(out_pos as isize),
                    &mut out_len,
                    flags,
                );
                return (
                    status,
                    // Bytes read in this function + bytes read in tinfl_decompress
                    (in_buf.len() - in_iter.len()) + in_len,
                    out_pos + out_len - out_buf_start_pos
                );
            },
        };

        match action {
            Action::Next => r.state += 1,
            Action::Jump(state) => r.state = state,
            Action::End(status) => break status,
        }
    };

    let mut undo_bytes = 0;
    if status != TINFLStatus::NeedsMoreInput && status != TINFLStatus::FailedCannotMakeProgress {
        undo_bytes = cmp::min((r.num_bits >> 3) as usize, in_buf.len() - in_iter.len());
        r.num_bits -= (undo_bytes << 3) as u32;
    }

    r.bit_buf &= ((1u64 << r.num_bits) - 1) as BitBuffer;

    let need_adler = flags & (TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_COMPUTE_ADLER32) != 0;
    if need_adler && status as i32 >= 0 {
        r.check_adler32 = ::mz_adler32_oxide(
            r.check_adler32,
            &out_buf.get_ref()[out_buf_start_pos..out_buf_start_pos + out_buf.position() as usize]
        );

        if status == TINFLStatus::Done && flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 && r.check_adler32 != r.z_adler32 {
            status = TINFLStatus::Adler32Mismatch;
        }
    }

    // NOTE: Status here and in miniz_tester doesn't seem to match.
    (status, in_buf.len() - in_iter.len() - undo_bytes, out_buf.position() as usize)
}


#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

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

        assert_eq!(a_status, b_status);
        assert_eq!(a_buf[..a_status.2], b"Hello, zlib!"[..]);
        assert_eq!(b_buf[..b_status.2], b"Hello, zlib!"[..]);
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
}
