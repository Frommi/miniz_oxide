use std::io::Cursor;
use std::{mem, cmp};
use crate::{MZResult, MZFlush, MZError, MZStatus};
use crate::inflate::TINFLStatus;
use crate::inflate::core::{DecompressorOxide, TINFL_LZ_DICT_SIZE, inflate_flags, decompress};

pub struct InflateState {
    decomp: DecompressorOxide,

    dict_ofs: usize,
    dict_avail: usize,
    first_call: u32,
    has_flushed: u32,

    zlib_header: bool,
    dict: [u8; TINFL_LZ_DICT_SIZE],
    last_status: TINFLStatus,
}

impl Default for InflateState {
    fn default() -> Self {
        InflateState {
            decomp: DecompressorOxide::default(),
            dict_ofs: 0,
            dict_avail: 0,
            first_call: 1,
            has_flushed: 0,

            zlib_header: false,
            dict: [0; TINFL_LZ_DICT_SIZE],
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


pub fn inflate(state: &mut InflateState, next_in: &mut &[u8], next_out: &mut &mut [u8],
           total_in: &mut u64, total_out: &mut u64,
           flush: MZFlush)
                           -> MZResult {
    if flush == MZFlush::Full {
        return Err(MZError::Stream);
    }

    let mut decomp_flags = inflate_flags::TINFL_FLAG_COMPUTE_ADLER32;
    if state.zlib_header {
        decomp_flags |= inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER;
    }

    let first_call = state.first_call;
    state.first_call = 0;
    if (state.last_status as i32) < 0 {
        return Err(MZError::Data);
    }

    if (state.has_flushed != 0) && (flush != MZFlush::Finish) {
        return Err(MZError::Stream);
    }
    state.has_flushed |= (flush == MZFlush::Finish) as u32;

    if (flush == MZFlush::Finish) && (first_call != 0) {
        decomp_flags |= inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        let mut cur = Cursor::new(mem::replace(next_out, &mut []));

        let status = decompress(
            &mut state.decomp,
            *next_in,
            &mut cur,
            decomp_flags,
        );
        let in_bytes = status.1;
        let out_bytes = status.2;
        let status = status.0;

        state.last_status = status;

        *next_in = &next_in[in_bytes..];
        //        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
        *next_out = &mut cur.into_inner()[out_bytes..];
        *total_in += in_bytes as u64;
        *total_out += out_bytes as u64;

        if (status as i32) < 0 {
            return Err(MZError::Data);
        } else if status != TINFLStatus::Done {
            state.last_status = TINFLStatus::Failed;
            return Err(MZError::Buf);
        }
        return Ok(MZStatus::StreamEnd);
    }

    if flush != MZFlush::Finish {
        decomp_flags |= inflate_flags::TINFL_FLAG_HAS_MORE_INPUT;
    }

    if state.dict_avail != 0 {
        *total_out += push_dict_out(state, next_out) as u64;
        return if (state.last_status == TINFLStatus::Done) &&
            (state.dict_avail == 0)
            {
                Ok(MZStatus::StreamEnd)
            } else {
            Ok(MZStatus::Ok)
        };
    }

    inflate_loop(state, next_in, next_out, total_in,
                 total_out, decomp_flags, flush)

}

fn inflate_loop(state: &mut InflateState, next_in: &mut &[u8], next_out: &mut &mut [u8],
                    total_in: &mut u64, total_out: &mut u64, decomp_flags: u32, flush: MZFlush)
                    -> MZResult {
    loop {
        let cursor_bytes;
        let status = {
            let mut out_cursor = Cursor::new(&mut state.dict[..]);
            out_cursor.set_position(state.dict_ofs as u64);
            let s = decompress(&mut state.decomp, *next_in, &mut out_cursor, decomp_flags);
            cursor_bytes = out_cursor.position() - state.dict_ofs as u64;
            s
        };

        let in_bytes = status.1;
        let out_bytes = status.2;
        let status = status.0;

        assert_eq!(out_bytes as u64, cursor_bytes);

        state.last_status = status;

        *next_in = &next_in[in_bytes..];
        *total_in += in_bytes as u64;

        state.dict_avail = out_bytes;
        *total_out += push_dict_out(state, next_out) as u64;

        if (status as i32) < 0 {
            return Err(MZError::Data);
        }

        // Note - compared to orig_in earlier but this should be the same.
        if (status == TINFLStatus::NeedsMoreInput) && next_in.is_empty() {
            return Err(MZError::Buf);
        }

        if flush == MZFlush::Finish {
            if status == TINFLStatus::Done {
                return if state.dict_avail != 0 {
                    Err(MZError::Buf)
                } else {
                    Ok(MZStatus::StreamEnd)
                };
            } else if next_out.is_empty() {
                return Err(MZError::Buf);
            }
        } else {
            let empty_buf = next_in.is_empty() || next_out.is_empty();
            if (status == TINFLStatus::Done) || empty_buf || (state.dict_avail != 0) {
                return if (status == TINFLStatus::Done) && (state.dict_avail == 0) {
                    Ok(MZStatus::StreamEnd)
                } else {
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
        let mut total_in = 0;
        let mut total_out = 0;
        let res = inflate(&mut state, &mut &encoded[..], &mut &mut out[..], &mut total_in,
                          &mut total_out, MZFlush::Finish);
        let status = res.expect("Failed to decompress!");
        assert_eq!(status, MZStatus::StreamEnd);
        assert_eq!(out[..total_out as usize], b"Hello, zlib!"[..]);
    }
}
