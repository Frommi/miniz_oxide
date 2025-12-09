#![cfg(feature = "with-alloc")]

use miniz_oxide::deflate::core::{compress_to_output, CompressorOxide, TDEFLFlush, TDEFLStatus};
use miniz_oxide::deflate::CompressionLevel;
use miniz_oxide::inflate::core::decompress;
use miniz_oxide::inflate::core::inflate_flags::*;
use miniz_oxide::inflate::core::DecompressorOxide;
use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::DataFormat;

/// Looks for byte-strings which when compressed result in each of the
/// final number of bits 0..=7, and then tests each of the flush modes
/// on that byte-string to see that it gives the correct result,
/// i.e. right number of bits added, and successful sync of the
/// stream.
#[test]
fn test_flush() {
    let mut found = 0;
    let mut n = 1;
    // Typically takes 59 iterations, but that could vary slightly if
    // compression algorithm is tweaked
    while found != 255 {
        let data = Rng::new(987654321).octal(n);
        n += 1;

        let base = compress(&data, &[TDEFLFlush::NoSync]);

        assert!(
            n != 1024,
            "BAILING OUT: Unexpected behaviour from compressor.  \
             Tested different 1024 compression-lengths, \
             and only these bit-lengths were found: {found:08b}. \
             For example: compressing {n} octal digits results in {} bytes + {} bits",
            base >> 3,
            base & 7
        );

        let mask = 1 << (base & 7);
        if (found & mask) != 0 {
            continue;
        }
        found |= mask;

        for nosync_first in [false, true] {
            for mode in [
                TDEFLFlush::Partial,
                TDEFLFlush::Sync,
                TDEFLFlush::Full,
                TDEFLFlush::Finish,
                TDEFLFlush::PartialOpt,
                TDEFLFlush::SyncOpt,
            ] {
                if nosync_first && mode == TDEFLFlush::Finish {
                    // `Finish` has to output a block even if empty to
                    // pass the finish flag, so skip as NoSync would
                    // be expected to change the output length in this
                    // case.  For all other cases doing a NoSync first
                    // should make no difference to the output.
                    continue;
                }
                let bits = if nosync_first {
                    compress(&data, &[TDEFLFlush::NoSync, mode])
                } else {
                    compress(&data, &[mode])
                };
                let expected = match mode {
                    TDEFLFlush::Partial => base + 10,
                    TDEFLFlush::Sync | TDEFLFlush::Full => {
                        // 3 bits, pad-to-byte, 16+16-bit length
                        ((base + 3 - 1) | 7) + 1 + 32
                    }
                    TDEFLFlush::Finish => {
                        // Pad-to-byte, Zlib trailer
                        ((base - 1) | 7) + 1 + 32
                    }
                    TDEFLFlush::PartialOpt => {
                        if (base & 7) != 0 {
                            base + 10
                        } else {
                            base
                        }
                    }
                    TDEFLFlush::SyncOpt => {
                        if (base & 7) != 0 {
                            ((base + 3 - 1) | 7) + 1 + 32
                        } else {
                            base
                        }
                    }
                    _ => panic!(),
                };

                assert_eq!(
                    bits,
                    expected,
                    "Unexpected flush behaviour for unwritten_bits={} \
                     mode={mode:?} nosync_first={nosync_first}: \
                     expecting {base} -> {expected}, but got {bits}",
                    base & 7
                );
            }
        }
    }
}

/// Test that a sync at start of a stream inserts the Zlib header
/// correctly
#[test]
fn check_zlib_with_sync_at_start() {
    // Test sync gets zlib header
    compress(b"", &[TDEFLFlush::Sync]);
    compress(b"", &[TDEFLFlush::Partial]);

    // Test that second sync DOESN'T get zlib header
    // (i.e. `block_index` handling is correct)
    compress(b"", &[TDEFLFlush::Sync, TDEFLFlush::Sync]);
    compress(b"", &[TDEFLFlush::Partial, TDEFLFlush::Partial]);
    compress(b"", &[TDEFLFlush::NoSync, TDEFLFlush::Sync]);
}

// Low-quality RNG, copied from test.rs
struct Rng(u64);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(((seed as u64) << 16) | 0x330E)
    }
    fn octal(&mut self, n: usize) -> Vec<u8> {
        self.map(|x| ((x & 7) + 48) as u8).take(n).collect()
    }
}

impl Iterator for Rng {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.0 = self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB);
        Some((self.0 >> 16) as u32)
    }
}

/// Compress data then apply the given flush modes in sequence and
/// return the number of output bits that result.  Also checks that
/// the decompression matches if the flush modes used are expected to
/// sync the stream.
fn compress(mut data: &[u8], modes: &[TDEFLFlush]) -> usize {
    let save_data = data;
    let mut compressor = CompressorOxide::new(0);
    compressor.set_format_and_level(DataFormat::Zlib, 0);
    compressor.set_compression_level(CompressionLevel::BestCompression);

    let mut out = Vec::new();
    loop {
        let mut ocount = 0;
        let (status, icount) =
            compress_to_output(&mut compressor, data, TDEFLFlush::None, |data| {
                ocount += data.len();
                out.extend_from_slice(data);
                true // Success
            });
        assert!(!matches!(
            status,
            TDEFLStatus::BadParam | TDEFLStatus::PutBufFailed
        ));
        data = &data[icount..];
        if icount == 0 && ocount == 0 {
            break;
        }
    }

    let mut check = false;
    for &mode in modes {
        let (status, _) = compress_to_output(&mut compressor, b"", mode, |data| {
            out.extend_from_slice(data);
            true // Success
        });
        assert!(!matches!(
            status,
            TDEFLStatus::BadParam | TDEFLStatus::PutBufFailed
        ));
        if !matches!(mode, TDEFLFlush::NoSync | TDEFLFlush::None) {
            check = true;
        }
    }

    if check {
        // Check that sync really does what it is supposed to be
        // doing, i.e. syncing the stream in the byte-stream output
        check_partial_inflate(&out, save_data);
    }

    out.len() * 8 + compressor.unwritten_bit_count() as usize
}

/// Check that an unterminated Zlib stream matches the given
/// uncompressed data, i.e. that it has synced correctly
fn check_partial_inflate(compressed: &[u8], uncompressed: &[u8]) {
    let mut out = Vec::new();
    let mut decompressor = DecompressorOxide::new();
    const DECODE_BUF_LEN: usize = 65536;
    let mut obuf = vec![0; DECODE_BUF_LEN];
    let mut iread = 0;
    let mut opos = 0;
    loop {
        let (status, icount, mut ocount) = decompress(
            &mut decompressor,
            &compressed[iread..],
            &mut obuf[..],
            opos,
            TINFL_FLAG_HAS_MORE_INPUT | TINFL_FLAG_PARSE_ZLIB_HEADER,
        );

        assert!(!matches!(
            status,
            TINFLStatus::FailedCannotMakeProgress
                | TINFLStatus::BadParam
                | TINFLStatus::Adler32Mismatch
                | TINFLStatus::Failed
        ));

        if icount == 0 && ocount == 0 {
            break;
        }

        iread += icount;

        while ocount > 0 {
            let count = ocount.min(obuf.len() - opos);
            out.extend_from_slice(&obuf[opos..opos + count]);
            opos = (opos + count) & (DECODE_BUF_LEN - 1);
            ocount -= count;
        }
    }

    assert_eq!(
        uncompressed,
        out.as_slice(),
        "Byte-stream doesn't decompress to the expected data"
    );
}
