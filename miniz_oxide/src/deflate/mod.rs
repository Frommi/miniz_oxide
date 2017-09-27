//! This module contains functionality for compressing data.

use libc::{c_int, c_void};

mod tdef_oxide;
pub use self::tdef_oxide::*;

pub type PutBufFuncPtrNotNull = unsafe extern "C" fn(*const c_void, c_int, *mut c_void) -> bool;
pub type PutBufFuncPtr = Option<PutBufFuncPtrNotNull>;

pub mod deflate_flags {
    /// Whether to use a zlib wrapper.
    pub const TDEFL_WRITE_ZLIB_HEADER: u32 = 0x0000_1000;
    /// Should we compute the adler32 checksum.
    pub const TDEFL_COMPUTE_ADLER32: u32 = 0x0000_2000;
    /// Should we use greedy parsing (as opposed to lazy parsing where look ahead one or more
    /// bytes to check for better matches.)
    pub const TDEFL_GREEDY_PARSING_FLAG: u32 = 0x0000_4000;
    /// TODO
    pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: u32 = 0x0000_8000;
    /// Only look for matches with a distance of 0.
    pub const TDEFL_RLE_MATCHES: u32 = 0x0001_0000;
    /// Only use matches that are at least 5 bytes long.
    pub const TDEFL_FILTER_MATCHES: u32 = 0x0002_0000;
    /// Force the compressor to only output static blocks. (Blocks using the default huffman codes
    /// specified in the deflate specification.)
    pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: u32 = 0x0004_0000;
    /// Force the compressor to only output raw/uncompressed blocks.
    pub const TDEFL_FORCE_ALL_RAW_BLOCKS: u32 = 0x0008_0000;
}

pub use self::deflate_flags::*;


/// How much processing the compressor should do to compress the data.
/// `NoCompression` and `Bestspeed` have special meanings, the other levels determine the number
/// of checks for matches in the hash chains and whether to use lazy or greedy parsing.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CompressionLevel {
    /// Don't do any compression, only output uncompressed blocks.
    NoCompression = 0,
    /// Fast compression. Uses a special compression routine that is optimized for speed.
    BestSpeed = 1,
    /// Slow/high compression. Do a lot of checks to try to find good matches.
    BestCompression = 9,
    /// Even more checks, can be very slow.
    UberCompression = 10,
    /// Default compromise between speed and compression.
    DefaultLevel = 6,
    /// Use the default compression level.
    DefaultCompression = -1,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CompressionStrategy {
    /// Don't use any of the special strategies.
    Default = 0,
    /// Only use matches that are at least 5 bytes long.
    Filtered = 1,
    /// Don't look for matches, only huffman encode the literals.
    HuffmanOnly = 2,
    /// Only look for matches with a distance of 1, i.e do run-length encoding only.
    RLE = 3,
    /// Only use static/fixed blocks. (Blocks using the default huffman codes
    /// specified in the deflate specification.)
    Fixed = 4,
}


// Missing safe rust analogue (this and mem-to-mem are quite similar)
/*
fn tdefl_compress(
    d: Option<&mut CompressorOxide>,
    in_buf: *const c_void,
    in_size: Option<&mut usize>,
    out_buf: *mut c_void,
    out_size: Option<&mut usize>,
    flush: TDEFLFlush,
) -> TDEFLStatus {
    let res = match d {
        None => {
            in_size.map(|size| *size = 0);
            out_size.map(|size| *size = 0);
            (TDEFLStatus::BadParam, 0, 0)
        },
        Some(compressor) => {
            let callback_res = CallbackOxide::new(
                compressor.callback_func.clone(),
                in_buf,
                in_size,
                out_buf,
                out_size,
            );

            if let Ok(mut callback) = callback_res {
                let res = compress(compressor, &mut callback, flush);
                callback.update_size(Some(res.1), Some(res.2));
                res
            } else {
                (TDEFLStatus::BadParam, 0, 0)
            }
        }
    };
    res.0
}*/

// Missing safe rust analogue
/*
fn tdefl_init(
    d: Option<&mut CompressorOxide>,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> TDEFLStatus {
    if let Some(d) = d {
        *d = CompressorOxide::new(
            put_buf_func.map(|func|
                CallbackFunc { put_buf_func: func, put_buf_user: put_buf_user }
            ),
            flags as u32,
        );
        TDEFLStatus::Okay
    } else {
        TDEFLStatus::BadParam
    }
}*/

// Missing safe rust analogue (though maybe best served by flate2 front-end instead)
/*
fn tdefl_compress_mem_to_output(
    buf: *const c_void,
    buf_len: usize,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> bool*/

// Missing safe Rust analogue
/*
fn tdefl_compress_mem_to_mem(
    out_buf: *mut c_void,
    out_buf_len: usize,
    src_buf: *const c_void,
    src_buf_len: usize,
    flags: c_int,
) -> usize*/

/// Compress the input data to a vector, using the specified compression level (0-10).
pub fn compress_to_vec(input: &[u8], level: u8) -> Vec<u8> {
    compress_to_vec_inner(input, level, false)
}

/// Compress the input data to a vector, using the specified compression level (0-10), and with a
/// zlib wrapper.
pub fn compress_to_vec_zlib(input: &[u8], level: u8) -> Vec<u8> {
    compress_to_vec_inner(input, level, true)
}

/// Simple function to compress data to a vec.
fn compress_to_vec_inner(input: &[u8], level: u8, with_zlib: bool) -> Vec<u8> {
    // The comp flags function sets the zlib flag if the window_bits parameter is > 0.
    let flags = create_comp_flags_from_zip_params(level.into(), with_zlib as i32, 0);
    let mut compressor = CompressorOxide::new(None, flags);
    let mut output = Vec::with_capacity(input.len() / 2);
    // # Unsafe
    // We trust compress to not read the uninitialized bytes.
    unsafe {
        let cap = output.capacity();
        output.set_len(cap);
    }
    let mut in_pos = 0;
    let mut out_pos = 0;
    loop {
        let (status, bytes_in, bytes_out) =
            compress(
                &mut compressor,
                &mut CallbackOxide::new_callback_buf(&input[in_pos..], &mut output[out_pos..]),
                TDEFLFlush::Finish,
            );

        out_pos += bytes_out;
        in_pos += bytes_in;

        match status {
            TDEFLStatus::Done => {
                output.truncate(out_pos);
                break;
            }
            TDEFLStatus::Okay => {
                // We need more space, so extend the vector.
                if output.len().saturating_sub(out_pos) < 30 {
                    let current_len = output.len();
                    output.reserve(current_len);

                    // # Unsafe
                    // We trust compress to not read the uninitialized bytes.
                    unsafe {
                        let cap = output.capacity();
                        output.set_len(cap);
                    }
                }
            }
            // Not supposed to happen unless there is a bug.
            _ => panic!("Bug! Unexpectedly failed to compress!"),
        }
    }

    output
}


#[cfg(test)]
mod test {
    use super::compress_to_vec;

    /// Test deflate example.
    ///
    /// Check if the encoder produces the same code as the example given by Mark Adler here:
    /// https://stackoverflow.com/questions/17398931/deflate-encoding-with-static-huffman-codes/17415203
    #[test]
    fn compress_small() {
        let test_data = b"Deflate late";
        let check = [0x73, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x00];

        let res = compress_to_vec(test_data, 9);
        assert_eq!(&check[..], res.as_slice());
    }
}
