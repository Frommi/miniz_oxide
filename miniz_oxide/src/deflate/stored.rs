use crate::deflate::core::{
    flush_block, CallbackOxide, CompressorOxide, TDEFLFlush, TDEFLStatus, LZ_DICT_SIZE,
    LZ_DICT_SIZE_MASK, MAX_MATCH_LEN,
};
use core::cmp;

/// Maximum number of bytes buffered in a stored block before flushing.
const STORED_BLOCK_SIZE: usize = 31 * 1024 + 1;

fn copy_to_dict(d: &mut CompressorOxide, mut pos: usize, mut input: &[u8]) {
    while !input.is_empty() {
        let dst = pos & LZ_DICT_SIZE_MASK;
        let len = cmp::min(input.len(), LZ_DICT_SIZE - dst);
        d.dict.b.dict[dst..dst + len].copy_from_slice(&input[..len]);

        // Mirror the start of the dictionary for reads that cross the ring boundary.
        if dst < MAX_MATCH_LEN - 1 {
            let mirrored = cmp::min(len, MAX_MATCH_LEN - 1 - dst);
            d.dict.b.dict[LZ_DICT_SIZE + dst..LZ_DICT_SIZE + dst + mirrored]
                .copy_from_slice(&input[..mirrored]);
        }

        pos += len;
        input = &input[len..];
    }
}

fn flush_stored_block(
    d: &mut CompressorOxide,
    callback: &mut CallbackOxide,
    bytes_written: usize,
    src_pos: usize,
    lookahead_size: usize,
    lookahead_pos: usize,
) -> i32 {
    d.lz.total_bytes = bytes_written as u32;
    d.params.src_pos = src_pos;
    d.dict.lookahead_size = lookahead_size;
    d.dict.lookahead_pos = lookahead_pos;

    flush_block(d, callback, TDEFLFlush::None).unwrap_or(TDEFLStatus::PutBufFailed as i32)
}

/// Compression function for stored blocks, split out from the main compression function.
pub(crate) fn compress_stored(d: &mut CompressorOxide, callback: &mut CallbackOxide) -> bool {
    let in_buf = match callback.buf() {
        None => return true,
        Some(in_buf) => in_buf,
    };

    // Right now there isn't any code for re-adding previous data to the hash chain if compression is switched
    // from stored mode to level 1 or higher after first starting compression at level 0
    // which causes a slight deviation from oroginal miniz behaviour but much faster
    // stored compression since original miniz does this by being super inefficient
    // and adds data to hash table when doing stored and rle compression.
    // Ideally we would handle it like zlib which keeps track of previous data and
    // only adds to hash table when switching over to actual compression but that
    // would need a bunch of more work.

    // Make sure this is cleared in case compression level is switched later.
    // TODO: It's possible we don't need this or could do this elsewhere later
    // but just do this here to avoid causing issues for now.
    d.params.saved_match_len = 0;
    let mut bytes_written = d.lz.total_bytes as usize;
    let mut src_pos = d.params.src_pos;
    let mut lookahead_size = d.dict.lookahead_size;
    let mut lookahead_pos = d.dict.lookahead_pos;

    // Retain enough lookahead to allow switching from stored to compressed mode.
    let retain = if d.params.flush == TDEFLFlush::None {
        MAX_MATCH_LEN - 1
    } else {
        0
    };
    let mut process = (lookahead_size + in_buf.len() - src_pos).saturating_sub(retain);

    while process != 0 {
        let block_space = STORED_BLOCK_SIZE - bytes_written;
        let len = cmp::min(process, block_space);
        let buffered = cmp::min(len, lookahead_size);
        lookahead_size -= buffered;

        let from_input = len - buffered;
        copy_to_dict(
            d,
            lookahead_pos + buffered,
            &in_buf[src_pos..src_pos + from_input],
        );
        src_pos += from_input;
        lookahead_pos += len;
        bytes_written += len;
        d.dict.size = cmp::min(d.dict.size + len, LZ_DICT_SIZE);
        process -= len;

        if bytes_written == STORED_BLOCK_SIZE {
            let result = flush_stored_block(
                d,
                callback,
                bytes_written,
                src_pos,
                lookahead_size,
                lookahead_pos,
            );
            if result != 0 {
                return result > 0;
            }
            bytes_written = d.lz.total_bytes as usize;
        }
    }

    let remaining = in_buf.len() - src_pos;
    copy_to_dict(d, lookahead_pos + lookahead_size, &in_buf[src_pos..]);
    src_pos += remaining;
    lookahead_size += remaining;
    d.dict.size = cmp::min(d.dict.size, LZ_DICT_SIZE - lookahead_size);

    d.lz.total_bytes = bytes_written as u32;
    d.params.src_pos = src_pos;
    d.dict.lookahead_size = lookahead_size;
    d.dict.lookahead_pos = lookahead_pos;
    true
}

/*
fn compress_rle(d: &mut CompressorOxide, callback: &mut CallbackOxide) -> bool {
    let mut src_pos = d.params.src_pos;
    let in_buf = match callback.in_buf {
        None => return true,
        Some(in_buf) => in_buf,
    };

    let mut lookahead_size = d.dict.lookahead_size;
    let mut lookahead_pos = d.dict.lookahead_pos;
    let mut saved_lit = d.params.saved_lit;
    let mut saved_match_dist = d.params.saved_match_dist;
    let mut saved_match_len = d.params.saved_match_len;

    while src_pos < in_buf.len() || (d.params.flush != TDEFLFlush::None && lookahead_size != 0) {
        let src_buf_left = in_buf.len() - src_pos;
        let num_bytes_to_process = cmp::min(src_buf_left, MAX_MATCH_LEN - lookahead_size);

        if lookahead_size + d.dict.size >= usize::from(MIN_MATCH_LEN) - 1
            && num_bytes_to_process > 0
        {
            let dictb = &mut d.dict.b;

            let mut dst_pos = (lookahead_pos + lookahead_size) & LZ_DICT_SIZE_MASK;
            let mut ins_pos = lookahead_pos + lookahead_size - 2;
            // Start the hash value from the first two bytes
            let mut hash = update_hash(
                u16::from(dictb.dict[ins_pos & LZ_DICT_SIZE_MASK]),
                dictb.dict[(ins_pos + 1) & LZ_DICT_SIZE_MASK],
            );

            lookahead_size += num_bytes_to_process;

            for &c in &in_buf[src_pos..src_pos + num_bytes_to_process] {
                // Add byte to input buffer.
                dictb.dict[dst_pos] = c;
                if dst_pos < MAX_MATCH_LEN - 1 {
                    dictb.dict[LZ_DICT_SIZE + dst_pos] = c;
                }

                // Generate hash from the current byte,
                hash = update_hash(hash, c);
                dictb.next[ins_pos & LZ_DICT_SIZE_MASK] = dictb.hash[hash as usize];
                // and insert it into the hash chain.
                dictb.hash[hash as usize] = ins_pos as u16;
                dst_pos = (dst_pos + 1) & LZ_DICT_SIZE_MASK;
                ins_pos += 1;
            }
            src_pos += num_bytes_to_process;
        } else {
            let dictb = &mut d.dict.b;
            for &c in &in_buf[src_pos..src_pos + num_bytes_to_process] {
                let dst_pos = (lookahead_pos + lookahead_size) & LZ_DICT_SIZE_MASK;
                dictb.dict[dst_pos] = c;
                if dst_pos < MAX_MATCH_LEN - 1 {
                    dictb.dict[LZ_DICT_SIZE + dst_pos] = c;
                }

                lookahead_size += 1;
                if lookahead_size + d.dict.size >= MIN_MATCH_LEN.into() {
                    let ins_pos = lookahead_pos + lookahead_size - 3;
                    let hash = ((u32::from(dictb.dict[ins_pos & LZ_DICT_SIZE_MASK])
                        << (LZ_HASH_SHIFT * 2))
                        ^ ((u32::from(dictb.dict[(ins_pos + 1) & LZ_DICT_SIZE_MASK])
                            << LZ_HASH_SHIFT)
                            ^ u32::from(c)))
                        & (LZ_HASH_SIZE as u32 - 1);

                    dictb.next[ins_pos & LZ_DICT_SIZE_MASK] = dictb.hash[hash as usize];
                    dictb.hash[hash as usize] = ins_pos as u16;
                }
            }

            src_pos += num_bytes_to_process;
        }

        d.dict.size = cmp::min(LZ_DICT_SIZE - lookahead_size, d.dict.size);
        if d.params.flush == TDEFLFlush::None && lookahead_size < MAX_MATCH_LEN {
            break;
        }

        let mut len_to_move = 1;
        let mut cur_match_dist = 0;
        let mut cur_match_len = if saved_match_len != 0 {
            saved_match_len
        } else {
            u32::from(MIN_MATCH_LEN) - 1
        };
        let cur_pos = lookahead_pos & LZ_DICT_SIZE_MASK;
                // If TDEFL_RLE_MATCHES is set, we only look for repeating sequences of the current byte.
        if d.dict.size != 0 && d.params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS == 0 {
            let c = d.dict.b.dict[(cur_pos.wrapping_sub(1)) & LZ_DICT_SIZE_MASK];
                    cur_match_len = d.dict.b.dict[cur_pos..(cur_pos + lookahead_size)]
                        .iter()
                        .take_while(|&x| *x == c)
                        .count() as u32;
                    if cur_match_len < MIN_MATCH_LEN.into() {
                        cur_match_len = 0
                    } else {
                        cur_match_dist = 1
                    }
                }


        let far_and_small = cur_match_len == MIN_MATCH_LEN.into() && cur_match_dist >= 8 * 1024;
        let filter_small = d.params.flags & TDEFL_FILTER_MATCHES != 0 && cur_match_len <= 5;
        if far_and_small || filter_small || cur_pos == cur_match_dist as usize {
            cur_match_dist = 0;
            cur_match_len = 0;
        }

        if saved_match_len != 0 {
            if cur_match_len > saved_match_len {
                record_literal(&mut d.huff, &mut d.lz, saved_lit);
                if cur_match_len >= 128 {
                    record_match(&mut d.huff, &mut d.lz, cur_match_len, cur_match_dist);
                    saved_match_len = 0;
                    len_to_move = cur_match_len as usize;
                } else {
                    saved_lit = d.dict.b.dict[cur_pos];
                    saved_match_dist = cur_match_dist;
                    saved_match_len = cur_match_len;
                }
            } else {
                record_match(&mut d.huff, &mut d.lz, saved_match_len, saved_match_dist);
                len_to_move = (saved_match_len - 1) as usize;
                saved_match_len = 0;
            }
        } else if cur_match_dist == 0 {
            record_literal(
                &mut d.huff,
                &mut d.lz,
                d.dict.b.dict[cmp::min(cur_pos, d.dict.b.dict.len() - 1)],
            );
        } else if d.params.greedy_parsing
            || (d.params.flags & TDEFL_RLE_MATCHES != 0)
            || cur_match_len >= 128
        {
            // If we are using lazy matching, check for matches at the next byte if the current
            // match was shorter than 128 bytes.
            record_match(&mut d.huff, &mut d.lz, cur_match_len, cur_match_dist);
            len_to_move = cur_match_len as usize;
        } else {
            saved_lit = d.dict.b.dict[cmp::min(cur_pos, d.dict.b.dict.len() - 1)];
            saved_match_dist = cur_match_dist;
            saved_match_len = cur_match_len;
        }

        lookahead_pos += len_to_move;
        assert!(lookahead_size >= len_to_move);
        lookahead_size -= len_to_move;
        d.dict.size = cmp::min(d.dict.size + len_to_move, LZ_DICT_SIZE);

        let lz_buf_tight = d.lz.code_position > LZ_CODE_BUF_SIZE - 8;
        let raw = d.params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0;
        let fat = ((d.lz.code_position * 115) >> 7) >= d.lz.total_bytes as usize;
        let fat_or_raw = (d.lz.total_bytes > 31 * 1024) && (fat || raw);

        if lz_buf_tight || fat_or_raw {
            d.params.src_pos = src_pos;
            // These values are used in flush_block, so we need to write them back here.
            d.dict.lookahead_size = lookahead_size;
            d.dict.lookahead_pos = lookahead_pos;

            let n = flush_block(d, callback, TDEFLFlush::None)
                .unwrap_or(TDEFLStatus::PutBufFailed as i32);
            if n != 0 {
                d.params.saved_lit = saved_lit;
                d.params.saved_match_dist = saved_match_dist;
                d.params.saved_match_len = saved_match_len;
                return n > 0;
            }
        }
    }

    d.params.src_pos = src_pos;
    d.dict.lookahead_size = lookahead_size;
    d.dict.lookahead_pos = lookahead_pos;
    d.params.saved_lit = saved_lit;
    d.params.saved_match_dist = saved_match_dist;
    d.params.saved_match_len = saved_match_len;
    true
}*/
