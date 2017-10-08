//! This module contains functionality for decompression.
//! `decompress` is the main decompression function.

use std::{mem, usize};
use std::io::Cursor;

pub mod core;
mod output_buffer;
use self::core::*;


/// Decompress the deflate-encoded data in `input` to a vector.
///
/// Returns a status and an integer representing where the decompressor failed on failure.
#[inline]
pub fn decompress_to_vec(input: &[u8]) -> Result<Vec<u8>, TINFLStatus> {
    decompress_to_vec_inner(input, 0)
}

/// Decompress the deflate-encoded data (with a zlib wrapper) in `input` to a vector.
///
/// Returns a status and an integer representing where the decompressor failed on failure.
#[inline]
pub fn decompress_to_vec_zlib(input: &[u8]) -> Result<Vec<u8>, TINFLStatus> {
    decompress_to_vec_inner(input, inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER)
}

fn decompress_to_vec_inner(input: &[u8], flags: u32) -> Result<Vec<u8>, TINFLStatus> {
    let flags = flags | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let mut ret = Vec::with_capacity(input.len() * 2);

    // # Unsafe
    // We trust decompress to not read the unitialized bytes as it's wrapped
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
            decompress(&mut decomp, &input[in_pos..], &mut c, flags)
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
                // We trust decompress to not read the unitialized bytes as it's wrapped
                // in a cursor that's position is set to the end of the initialized data.
                unsafe {
                    let cap = ret.capacity();
                    ret.set_len(cap);
                }
            },

            _ => return Err(status),
        }
    }
}

#[cfg(test)]
mod test {
    use super::decompress_to_vec_zlib;

    #[test]
    fn decompress_vec() {
        let encoded = [
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168,
            202, 201, 76,  82,   4,   0,  27, 101,  4,  19,
        ];
        let res = decompress_to_vec_zlib(&encoded[..]).unwrap();
        assert_eq!(res.as_slice(), &b"Hello, zlib!"[..]);
    }
}
