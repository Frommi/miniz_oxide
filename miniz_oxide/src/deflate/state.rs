use std::mem;
use crate::{MZResult, MZFlush, MZError, MZStatus};
use crate::deflate::core::{TDEFLStatus, TDEFLFlush, CompressorOxide, compress};

pub fn deflate(compressor: &mut CompressorOxide, next_in: &mut &[u8],
               next_out: &mut &mut [u8],
               total_in: &mut u64, total_out: &mut u64,
               flush: MZFlush) -> MZResult {

    if next_out.is_empty() {
        return Err(MZError::Buf);
    }

    if compressor.prev_return_status() == TDEFLStatus::Done {
        return if flush == MZFlush::Finish {
            Ok(MZStatus::StreamEnd)
        } else {
            Err(MZError::Buf)
        };
    }

    let original_total_in = *total_in;
    let original_total_out = *total_out;

    loop {
        let in_bytes;
        let out_bytes;
        let defl_status = {
            let res = compress(compressor, *next_in, *next_out, TDEFLFlush::from(flush));
            in_bytes = res.1;
            out_bytes = res.2;
            res.0
        };

        *next_in = &next_in[in_bytes..];
        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
        *total_in += in_bytes as u64;
        *total_out += out_bytes as u64;

        if defl_status == TDEFLStatus::BadParam || defl_status == TDEFLStatus::PutBufFailed {
            return Err(MZError::Stream);
        }

        if defl_status == TDEFLStatus::Done {
            return Ok(MZStatus::StreamEnd);
        }

        if next_out.is_empty() {
            return Ok(MZStatus::Ok);
        }

        if next_in.is_empty() && (flush != MZFlush::Finish) {
            let total_changed = (*total_in != original_total_in) ||
                (*total_out != original_total_out);

            return if (flush != MZFlush::None) || total_changed {
                Ok(MZStatus::Ok)
            } else {
                Err(MZError::Buf)
            };
        }
    }
}
