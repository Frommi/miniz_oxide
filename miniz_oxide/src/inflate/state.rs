use std::io::Cursor;
use std::{mem, cmp};
use std::convert::{AsMut, AsRef};

use crate::{MZResult, MZFlush, MZError, MZStatus, StreamResult};
use crate::inflate::TINFLStatus;
use crate::inflate::core::{DecompressorOxide, TINFL_LZ_DICT_SIZE, inflate_flags, decompress};

pub struct InflateState {
    /// Inner decompressor struct
    decomp: DecompressorOxide,

    /// Buffer of input bytes for matches.
    /// TODO: Could probably do this a bit cleaner with some
    /// Cursor-like class.
    /// We may also look into whether we need to keep a buffer here, or just one in the
    /// decompressor struct.
    dict: [u8; TINFL_LZ_DICT_SIZE],
    /// Where in the buffer are we currently at?
    dict_ofs: usize,
    /// How many bytes of data to be flushed is there currently in the buffer?
    dict_avail: usize,

    first_call: bool,
    has_flushed: bool,

    /// Whether the input data is wrapped in a zlib header and checksum.
    /// TODO: This should be stored in the decompressor.
    zlib_header: bool,
    last_status: TINFLStatus,
}

impl Default for InflateState {
    fn default() -> Self {
        InflateState {
            decomp: DecompressorOxide::default(),
            dict: [0; TINFL_LZ_DICT_SIZE],
            dict_ofs: 0,
            dict_avail: 0,
            first_call: true,
            has_flushed: false,
            zlib_header: false,
            last_status: TINFLStatus::NeedsMoreInput,
        }
    }
}
impl InflateState {
    /// Create a new state.
    ///
    /// Parameters:
    /// `zlib_header`: Determines whether the compressed data is assumed to wrapped with zlib
    /// metadata.
    pub fn new_boxed(zlib_header: bool) -> Box<InflateState> {
        let mut b: Box<InflateState> = Box::default();
        b.zlib_header = zlib_header;
        b
    }

    pub fn decompressor(&mut self) -> &mut DecompressorOxide {
        &mut self.decomp
    }

    pub fn last_status(&self) -> TINFLStatus {
        self.last_status
    }

    /// Create a new state using miniz/zlib style window bits parameter.
    ///
    /// The decompressor does not support different window sizes. As such,
    /// any positive (>0) value will set the zlib header flag, while a negative one
    /// will not.
    pub fn new_boxed_with_window_bits(window_bits: i32) -> Box<InflateState> {
        let mut b: Box<InflateState> = Box::default();

        b.zlib_header = window_bits > 0;
        b
    }

}


pub fn inflate<I: AsRef<[u8]>, O: AsMut<[u8]>>(state: &mut InflateState, input: &I, output: &mut O,
           flush: MZFlush)
               -> StreamResult {
    println!("Called inflate!");
    let mut bytes_consumed = 0;
    let mut bytes_written = 0;
    let mut next_in = input.as_ref();
    let mut next_out = output.as_mut();

    if flush == MZFlush::Full {
        return StreamResult::error(MZError::Stream);
    }

    let mut decomp_flags = inflate_flags::TINFL_FLAG_COMPUTE_ADLER32;
    if state.zlib_header {
        decomp_flags |= inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER;
    }

    let first_call = state.first_call;
    state.first_call = false;
    if (state.last_status as i32) < 0 {
        println!("Data error: {:?}", state.last_status);
        return StreamResult::error(MZError::Data);
    }

    if state.has_flushed && (flush != MZFlush::Finish) {
        return StreamResult::error(MZError::Stream);
    }
    state.has_flushed |= flush == MZFlush::Finish;

    if (flush == MZFlush::Finish) && first_call {
        decomp_flags |= inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        let status = decompress(
            &mut state.decomp,
            next_in,
            &mut Cursor::new(next_out),
            decomp_flags,
        );
        let in_bytes = status.1;
        let out_bytes = status.2;
        let status = status.0;

        state.last_status = status;

        bytes_consumed += in_bytes;
        bytes_written += out_bytes;

        let ret_status = {
            if (status as i32) < 0 {
                println!("Decomp failed!: {:?}", state.last_status);
                Err(MZError::Data)
            } else if status != TINFLStatus::Done {
                println!("no space in buffer to flush finish!: {:?}", state.last_status);
                state.last_status = TINFLStatus::Failed;
                Err(MZError::Buf)
            } else {
                Ok(MZStatus::StreamEnd)
            }
        };
        return StreamResult {
            bytes_consumed,
            bytes_written,
            status:ret_status,
        };
    }

    if flush != MZFlush::Finish {
        decomp_flags |= inflate_flags::TINFL_FLAG_HAS_MORE_INPUT;
    }

    if state.dict_avail != 0 {
        bytes_written += push_dict_out(state, &mut next_out);
        return StreamResult {
            bytes_consumed,
            bytes_written,
            status: Ok(
                if (state.last_status == TINFLStatus::Done) && (state.dict_avail == 0) {
                    MZStatus::StreamEnd
                } else {
                    MZStatus::Ok
                }
            )
        };
    }

    let status = inflate_loop(state, &mut next_in, &mut next_out, &mut bytes_consumed,
                              &mut bytes_written, decomp_flags, flush);
    StreamResult{
        bytes_consumed,
        bytes_written,
        status,
    }
}

fn inflate_loop(state: &mut InflateState, next_in: &mut &[u8], next_out: &mut &mut [u8],
                    total_in: &mut usize, total_out: &mut usize, decomp_flags: u32, flush: MZFlush)
                -> MZResult {
    let orig_in_len = next_in.len();
    loop {
        let status = {
            let mut cursor = Cursor::new(&mut state.dict[..]);
            cursor.set_position(state.dict_ofs as u64);
            decompress(&mut state.decomp, *next_in,
                                 &mut cursor, decomp_flags)
        };

        let in_bytes = status.1;
        let out_bytes = status.2;
        let status = status.0;

        state.last_status = status;

        *next_in = &next_in[in_bytes..];
        *total_in += in_bytes;

        state.dict_avail = out_bytes;
        *total_out += push_dict_out(state, next_out);

        // The stream was corrupted, and decompression failed.
        if (status as i32) < 0 {
            println!("Decomp failed loop!: {:?}", state.last_status);
            return Err(MZError::Data);
        }

        // The decompressor has flushed all it's data and is waiting for more input, but
        // there was no more input provided.
        if (status == TINFLStatus::NeedsMoreInput) && orig_in_len == 0 {
            return Err(MZError::Buf);
        }

        if flush == MZFlush::Finish {
            if status == TINFLStatus::Done {
                // There is not enough space in the output buffer to flush the remaining
                // decompressed data in the internal buffer.
                return if state.dict_avail != 0 {
                    Err(MZError::Buf)
                } else {
                    Ok(MZStatus::StreamEnd)
                };
                // No more space in the output buffer, but we're not done.
            } else if next_out.is_empty() {
                return Err(MZError::Buf);
            }
        } else {
            // We're not expected to finish, so it's fine if we can't flush everything yet.
            let empty_buf = next_in.is_empty() || next_out.is_empty();
            if (status == TINFLStatus::Done) || empty_buf || (state.dict_avail != 0) {
                return if (status == TINFLStatus::Done) && (state.dict_avail == 0) {
                    // No more data left, we're done.
                    Ok(MZStatus::StreamEnd)
                } else {
                    // Ok for now, still waiting for more input data or output space.
                    Ok(MZStatus::Ok)
                };
            }
        }
    }
}


fn push_dict_out(state: &mut InflateState, next_out: &mut &mut [u8]) -> usize {
    let n = cmp::min(state.dict_avail as usize, next_out.len());
    (next_out[..n]).copy_from_slice(
        &state.dict[state.dict_ofs..state.dict_ofs + n],
    );
    *next_out = &mut mem::replace(next_out, &mut [])[n..];
    state.dict_avail -= n;
    state.dict_ofs = (state.dict_ofs + (n)) &
        ((TINFL_LZ_DICT_SIZE - 1));
    n
}

#[cfg(test)]
mod test {
    use super::{InflateState, inflate};
    use crate::{MZFlush, MZStatus};
    #[test]
    fn test_state() {
        let encoded = [
            120u8, 156, 243, 72, 205, 201, 201, 215, 81, 168,
            202, 201,  76,  82,  4,   0,  27, 101,  4,  19,
        ];
        let mut out = vec![0; 50];
        let mut state = InflateState::new_boxed(true);
        let res = inflate(&mut state, &encoded, &mut out, MZFlush::Finish);
        let status = res.status.expect("Failed to decompress!");
        assert_eq!(status, MZStatus::StreamEnd);
        assert_eq!(out[..res.bytes_written as usize], b"Hello, zlib!"[..]);
        assert_eq!(res.bytes_consumed, encoded.len());
    }
}
