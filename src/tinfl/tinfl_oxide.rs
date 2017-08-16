use super::*;

enum Action {
    Next,
    End(TINFLStatus),
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
        out_buf.get_ref().len() - 1
    };

    if out_buf_size_mask & out_buf_size_mask.wrapping_add(1) != 0 {
        return (TINFLStatus::BadParam, 0, 0)
    }

    let mut in_buf = Cursor::new(in_buf);
    let status = loop {
        match match r.state {
            0 => {
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

            1 => {
                // TODO: check that read_exact simplifies to ~one instruction
                let mut one_byte = &mut [0u8; 1];
                match in_buf.read_exact(one_byte) {
                    Err(_) => {
                        Action::End(
                            if flags & TINFL_FLAG_HAS_MORE_INPUT != 0 {
                                TINFLStatus::NeedsMoreInput
                            } else {
                                TINFLStatus::FailedCannotMakeProgress
                            }
                        )
                    },
                    Ok(_) => {
                        r.z_header0 = one_byte[0] as u32;
                        Action::Next
                    },
                }
            },

            _ => unsafe {
                let in_pos = in_buf.position() as usize;
                let mut in_len = in_buf.get_ref().len() - in_pos;
                let out_pos = out_buf.position() as usize;
                let mut out_len = out_buf.get_ref().len() - out_pos;
                let out_buf = out_buf.get_mut();
                let status = tinfl_decompress(
                    r,
                    &in_buf.get_ref()[in_pos],
                    &mut in_len,
                    &mut out_buf[0],
                    &mut out_buf[out_pos],
                    &mut out_len,
                    flags,
                );
                return (status, in_pos + in_len, out_pos + out_len);
            },
        } {
            Action::Next => r.state += 1,
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

    (status, in_buf.position() as usize, out_buf.position() as usize)
}
