use super::*;

const START: u32 = 0;
const READ_ZLIB_CMF: u32 = 1;
const READ_ZLIB_FLG: u32 = 2;
const READ_BLOCK_HEADER: u32 = 3;
const BAD_ZLIB_HEADER: u32 = 36;

#[inline]
/// Check that the zlib header is correct and that there is enough space in the buffer
/// for the window size specified in the header.
///
/// See https://tools.ietf.org/html/rfc1950
fn validate_zlib_header(cmf: u32, flg: u32, flags: u32, mask: usize) -> bool {
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
    !failed
}


enum Action {
    Next,
    // Temporary until we have ported everything.
    RunMiniz(TINFLStatus, u32),
    End(TINFLStatus),
}

#[inline]
fn end_of_input(flags: u32) -> Action {
    // We haven't implemented an analogue to common_exit yet,
    // so crash here for now.
    Action::End(if flags & TINFL_FLAG_HAS_MORE_INPUT != 0 {
        TINFLStatus::NeedsMoreInput
    } else {
        TINFLStatus::FailedCannotMakeProgress
    });
    unimplemented!();
}

pub fn decompress_oxide(
    r: &mut tinfl_decompressor,
    in_buf: &[u8],
    out_buf: &mut Cursor<&mut [u8]>,
    flags: u32,
) -> (TINFLStatus, usize, usize) {
    let out_buf_size_mask = if flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0 {
        usize::max_value()
    } else {
        out_buf.position() as usize + out_buf.get_ref().len() - 1
    };

    // Ensure the output buffer's size is a power of 2, unless the output buffer
    // is large enough to hold the entire output file (in which case it doesn't
    // matter).
    if (out_buf_size_mask.wrapping_add(1) & out_buf_size_mask) != 0 {
        return (TINFLStatus::BadParam, 0, 0)
    }

    let mut in_iter = in_buf.iter();
    let status = loop {
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
                Action::Next
            },

            READ_ZLIB_CMF => {
                match in_iter.next() {
                    None => {
                        end_of_input(flags)
                    },
                    Some(&cmf) => {
                        r.z_header0 = cmf as u32;
                        Action::Next
                    },
                }
            },

            READ_ZLIB_FLG => {
                match in_iter.next() {
                    None => {
                        end_of_input(flags)
                    },
                    Some(&flg) => {
                        r.z_header1 = flg as u32;
                        if validate_zlib_header(r.z_header0, r.z_header1, flags, out_buf_size_mask) {
                            // Not sure if setting counter is needed, but do it for now just in case.
                            r.counter = 0;
                            Action::Next
                        } else {
                            //r.state = BAD_ZLIB_HEADER;
                            r.counter = 1;
                            Action::RunMiniz(TINFLStatus::Failed, BAD_ZLIB_HEADER)
                        }
                    }
                }
            }

            // Let miniz deal with these until we have ported over the common_exit part.
            /*BAD_ZLIB_HEADER => {
                Action::End(TINFLStatus::Failed)
            }*/

            _ => unsafe {
                let mut in_len = in_iter.len();
                let out_pos = out_buf.position() as usize;
                let mut out_len = out_buf.get_ref().len() - out_pos;
                let out_buf = out_buf.get_mut();
                let status = tinfl_decompress(
                    r,
                    in_iter.as_slice().as_ptr(),
                    &mut in_len,
                    &mut out_buf[0],
                    &mut out_buf[out_pos],
                    &mut out_len,
                    flags,
                );
                return (
                    status,
                    // Bytes read in this function + bytes read in tinfl_decompress
                    (in_buf.len() - in_iter.len()) + in_len,
                    out_pos + out_len);
            },
        };

        match action {
            Action::Next => r.state += 1,
            Action::RunMiniz(_, state) => r.state = state,
            Action::End(status) => break status,
        }
    };

    if status != TINFLStatus::NeedsMoreInput && status != TINFLStatus::FailedCannotMakeProgress {
        // TODO: give back full unprocessed bytes from bit_buffer
    }

    let need_adler = flags & (TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_COMPUTE_ADLER32) != 0;
    if need_adler && status as i32 >= 0 {
        // TODO: check adler
    }

    (status, in_buf.len() - in_iter.len(), out_buf.position() as usize)
}


#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    fn tinfl_decompress_miniz<'i>(
        r: &mut tinfl_decompressor,
        input_buffer: &'i [u8],
        output_buffer: &mut [u8],
        flags: u32) ->
        (TINFLStatus, &'i [u8], usize) {
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
        flags: u32) -> (TINFLStatus, &'i [u8], usize) {
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
        let mut a_buf = vec![0;LEN];
        let mut b_buf = vec![0;LEN];

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
}
