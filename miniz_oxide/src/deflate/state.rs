use std::convert::{AsMut, AsRef};

use crate::{MZFlush, MZError, MZStatus, StreamResult};
use crate::deflate::core::{TDEFLStatus, TDEFLFlush, CompressorOxide, compress};

/// Try to compress from input to output with the given Compressor
pub fn deflate<I: AsRef<[u8]>, O: AsMut<[u8]>>(compressor: &mut CompressorOxide, input: &I,
               output: &mut O,
               flush: MZFlush) -> StreamResult {

    if output.as_mut().is_empty() {
        return StreamResult::error(MZError::Buf)
    }

    if compressor.prev_return_status() == TDEFLStatus::Done {
        return if flush == MZFlush::Finish {
            StreamResult {
                bytes_written: 0,
                bytes_consumed: 0,
                status: Ok(MZStatus::StreamEnd)
            }
        } else {
            StreamResult::error(MZError::Buf)
        };
    }

    let mut bytes_written = 0;
    let mut bytes_consumed = 0;

    let mut next_in = input.as_ref();
    let mut next_out = output.as_mut();

    let status = loop {
        let in_bytes;
        let out_bytes;
        let defl_status = {
            let res = compress(compressor, next_in, next_out, TDEFLFlush::from(flush));
            in_bytes = res.1;
            out_bytes = res.2;
            res.0
        };

        next_in = &next_in[in_bytes..];
        next_out = &mut next_out[out_bytes..];
        bytes_consumed += in_bytes;
        bytes_written += out_bytes;

        if defl_status == TDEFLStatus::BadParam || defl_status == TDEFLStatus::PutBufFailed {
            break Err(MZError::Stream);
        }

        if defl_status == TDEFLStatus::Done {
            break Ok(MZStatus::StreamEnd);

        }

        if next_out.is_empty() {
            break Ok(MZStatus::Ok);
        }

        if next_in.is_empty() && (flush != MZFlush::Finish) {
            let total_changed = bytes_written > 0 || bytes_consumed > 0;

            break if (flush != MZFlush::None) || total_changed {
                Ok(MZStatus::Ok)
            } else {
                Err(MZError::Buf)
            };
        }
    };
    StreamResult {
        bytes_consumed,
        bytes_written,
        status,
    }
}


#[cfg(test)]
mod test {
    use crate::{MZFlush, MZStatus};
    use crate::deflate::CompressorOxide;
    use crate::inflate::decompress_to_vec_zlib;
    use super::deflate;
    #[test]
    fn test_state() {
        let data = b"Hello zlib!";
        let mut compressed = vec![0; 50];
        let mut compressor = Box::<CompressorOxide>::default();
        let res = deflate(&mut compressor, data, &mut compressed, MZFlush::Finish);
        let status = res.status.expect("Failed to compress!");
        let decomp = decompress_to_vec_zlib(&compressed)
            .expect("Failed to decompress compressed data");
        assert_eq!(status, MZStatus::StreamEnd);
        assert_eq!(decomp[..], data[..]);
        assert_eq!(res.bytes_consumed, data.len());
    }
}
