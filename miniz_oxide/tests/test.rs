// Disable these tests for now unless alloc is enabled since we're only testing with the *_vec functions in here.
#![cfg(feature = "with-alloc")]
extern crate miniz_oxide;

use std::io::Read;

use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};
use miniz_oxide::inflate::{decompress_to_vec, decompress_to_vec_zlib, TINFLStatus};
use miniz_oxide::MZError;

fn get_test_file_data(name: &str) -> Vec<u8> {
    use std::fs::File;
    let mut input = Vec::new();
    let mut f = File::open(name).unwrap();

    f.read_to_end(&mut input).unwrap();
    input
}

// Low-quality RNG to generate incompressible test data, based on mrand48
#[cfg(feature = "block-boundary")]
struct Rng(u64);

#[cfg(feature = "block-boundary")]
impl Rng {
    fn new(seed: u32) -> Self {
        Self(((seed as u64) << 16) | 0x330E)
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        self.flat_map(|x| x.to_le_bytes()).take(n).collect()
    }
}

#[cfg(feature = "block-boundary")]
impl Iterator for Rng {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.0 = self.0.wrapping_mul(0x5DEECE66D).wrapping_add(0xB);
        Some((self.0 >> 16) as u32)
    }
}

/// Fuzzed file that caused issues for the inflate library.
#[test]
fn inf_issue_14() {
    let data = get_test_file_data("tests/test_data/issue_14.zlib");
    let result = decompress_to_vec_zlib(data.as_slice());
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.status, TINFLStatus::Failed);
}

/// Fuzzed file that causes panics (subtract-with-overflow in debug, out-of-bounds in release)
#[test]
fn inf_issue_19() {
    let data = get_test_file_data("tests/test_data/issue_19.deflate");
    let _ = decompress_to_vec(data.as_slice());
}

/// Fuzzed (invalid )file that resulted in an infinite loop as inflate read a code as having 0
/// length.
#[test]
fn decompress_zero_code_len_oom() {
    let data = get_test_file_data("tests/test_data/invalid_code_len_oom");
    let _ = decompress_to_vec(data.as_slice());
}

/// Same problem as previous test but in the end of input huffman decode part of
/// `decode_huffman_code`
#[test]
fn decompress_zero_code_len_2() {
    let data = get_test_file_data("tests/test_data/invalid_code_len_oom");
    let _ = decompress_to_vec(data.as_slice());
}

fn get_test_data() -> Vec<u8> {
    use std::env;
    let path = env::var("TEST_FILE").unwrap_or_else(|_| "../miniz/miniz.c".to_string());
    get_test_file_data(&path)
}

fn roundtrip(level: u8) {
    let data = get_test_data();
    let enc = compress_to_vec(data.as_slice(), level);
    println!(
        "Input len: {}, compressed len: {}, level: {}",
        data.len(),
        enc.len(),
        level
    );
    let dec = decompress_to_vec(enc.as_slice()).unwrap();
    assert!(data == dec);
}

#[test]
fn roundtrip_lvl_9() {
    roundtrip(9);
}

#[test]
fn roundtrip_lvl_1() {
    roundtrip(1);
}

#[test]
fn roundtrip_lvl_0() {
    roundtrip(0);
}

#[test]
fn zlib_header_level() {
    let level = 6;
    let data = [1, 2, 3];
    let enc = compress_to_vec_zlib(&data, level);
    let header_level = (enc[1] & 0b11000000) >> 6;
    assert_eq!(header_level, 2);
    let enc = compress_to_vec_zlib(&data, 10);
    let header_level = (enc[1] & 0b11000000) >> 6;
    assert_eq!(header_level, 3);
}

#[test]
fn need_more_input_has_more_output_at_same_time() {
    use miniz_oxide::inflate::core;

    let input = get_test_file_data("tests/test_data/numbers.deflate");
    let data = get_test_file_data("tests/test_data/numbers.txt");

    let decomp = |input: &[u8]| {
        let mut decomp = core::DecompressorOxide::new();
        decomp.init();

        let mut output = [0; core::TINFL_LZ_DICT_SIZE];
        let flags = core::inflate_flags::TINFL_FLAG_HAS_MORE_INPUT;

        let (status, in_consumed, out_consumed) =
            core::decompress(&mut decomp, input, &mut output, 0, flags);

        let input_empty = in_consumed == input.len();
        let output_full = out_consumed == output.len();

        eprintln!(
            "input len: {}, input_empty: {:?}, output_full: {:?}, status: {:?}",
            input.len(),
            input_empty,
            output_full,
            status
        );

        match (input_empty, output_full) {
            (false, false) => unreachable!("Shouldn't happen in this test case."),
            (true, false) => assert_eq!(status, TINFLStatus::NeedsMoreInput),
            (false, true) => assert_eq!(status, TINFLStatus::HasMoreOutput),
            // NOTE: In case both "NeedsMoreInput" and "HasMoreOutput" are both true,
            // HasMoreOutput should be preferred as the user generally wants to
            // read output data before overwriting the buffer with more.
            (true, true) => assert_eq!(status, TINFLStatus::HasMoreOutput),
        }

        assert_eq!(&data[..out_consumed], &output[..out_consumed]);
    };

    // The last "clear" cases in the upper and lower limit
    decomp(&input[..11730]); // Ok; input_empty: false, output_full: true, status: HasMoreOutput
    decomp(&input[..11725]); // Ok; input_empty: true, output_full: false, status: NeedsMoreInput

    // A case where both buffers are full but the status is correct
    decomp(&input[..11729]); // Ok; input_empty: true, output_full: true, status: HasMoreOutput

    // Cases where both buffers are full but the status is incorrect
    decomp(&input[..11726]); // Fail: NeedsMoreInput even if the output buffer is also full!
    decomp(&input[..11727]); // Fail: NeedsMoreInput even if the output buffer is also full!
    decomp(&input[..11728]); // Fail: NeedsMoreInput even if the output buffer is also full!
}

#[test]
fn issue_75_empty_input_infinite_loop() {
    // Make sure compression works with empty input,
    // a bug resulted in this causing an infinite loop in
    // compress_to_vec_inner.
    let c = miniz_oxide::deflate::compress_to_vec(&[], 6);
    let d = miniz_oxide::inflate::decompress_to_vec(&c).expect("decompression failed!");
    assert_eq!(d.len(), 0);
    let c = miniz_oxide::deflate::compress_to_vec(&[0], 6);
    let d = miniz_oxide::inflate::decompress_to_vec(&c).expect("decompression failed!");
    assert!(d == [0]);
}

#[test]
fn issue_119_inflate_with_exact_limit() {
    use miniz_oxide::inflate::{decompress_to_vec_zlib, decompress_to_vec_zlib_with_limit};

    let compressed_data = &[
        120, 156, 237, 217, 65, 17, 194, 0, 16, 192, 192, 122, 193, 94, 13, 240, 232, 128, 12, 28,
        160, 2, 53, 53, 130, 139, 220, 227, 118, 21, 228, 159, 227, 13, 0, 212, 126, 211, 1, 0,
        176, 208, 99, 58, 0, 0, 22, 122, 78, 7, 0, 192, 66, 223, 233, 0, 0, 88, 200, 255, 5, 128,
        158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0, 208,
        243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0, 122,
        254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64, 207,
        255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0, 232, 249,
        191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11, 0, 61, 255,
        23, 0, 122, 254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255,
        2, 64, 207, 255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0,
        232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11,
        0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1,
        160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0,
        244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5,
        128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 247, 116, 0, 0, 44, 116, 78, 7, 0, 192, 66, 215,
        116, 0, 0, 44, 244, 154, 14, 0, 128, 133, 62, 211, 1, 0, 176, 144, 255, 11, 0, 61, 255, 23,
        0, 122, 254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2,
        64, 207, 255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0,
        232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11,
        0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1,
        160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0,
        244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5,
        128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0,
        208, 243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0,
        122, 254, 47, 0, 244, 252, 95, 0, 232, 249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64,
        207, 255, 5, 128, 158, 255, 11, 0, 61, 255, 23, 0, 122, 254, 47, 0, 244, 252, 95, 0, 232,
        249, 191, 0, 208, 243, 127, 1, 160, 231, 255, 2, 64, 207, 255, 5, 128, 158, 255, 11, 0, 61,
        255, 23, 0, 122, 254, 47, 0, 244, 254, 53, 209, 27, 197,
    ];

    let decompressed_size = decompress_to_vec_zlib(compressed_data)
        .expect("test is not valid, data must correctly decompress when not limited")
        .len();

    let _ = decompress_to_vec_zlib_with_limit(compressed_data, decompressed_size).unwrap_or_else(
        |_| {
            panic!(
                "data decompression failed when limited to {}",
                decompressed_size
            )
        },
    );
}

#[test]
fn issue_130_reject_invalid_table_sizes() {
    let input = get_test_file_data("tests/test_data/issue_130_table_size.bin");

    let result = decompress_to_vec_zlib(input.as_slice());
    println!("{:?}", result);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.status, TINFLStatus::Failed);
}

#[test]
fn issue_143_return_buf_error_on_finish_without_end_header() {
    use miniz_oxide::inflate::stream::{inflate, InflateState};
    use miniz_oxide::{DataFormat, MZFlush};

    let mut v1 = Vec::new();
    v1.extend_from_slice(&[0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00]);
    v1.extend_from_slice(&[0, 0, 0xFF, 0xFF]);

    let result = decompress_to_vec(v1.as_slice());
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.status, TINFLStatus::FailedCannotMakeProgress);

    let mut inflate_stream = InflateState::new(DataFormat::Raw);
    let mut output = vec![0u8; 30];

    let inflate_result = inflate(
        &mut inflate_stream,
        v1.as_slice(),
        &mut output,
        MZFlush::Finish,
    );

    assert_eq!(inflate_result.status.unwrap_err(), MZError::Buf)
}

#[test]
fn decompress_empty_dynamic() {
    // Empty block with dynamic huffman codes.
    let enc = vec![5, 192, 129, 8, 0, 0, 0, 0, 32, 127, 235, 0b011, 0, 0, 0];

    let res = decompress_to_vec(enc.as_slice()).unwrap();
    assert!(res.is_empty());

    let enc = vec![5, 192, 129, 8, 0, 0, 0, 0, 32, 127, 235, 0b1111011, 0, 0, 0];

    let res = decompress_to_vec(enc.as_slice());
    assert!(res.is_err());
}

#[test]
fn issue_169() {
    // Issue caused by to inflate returning MZError::Data instead of MZError::Buf
    // on incomplete stream when not called with finish on first call.
    // This caused flate2 to return error instead of ok on such streams.
    use miniz_oxide::inflate::stream::{inflate, InflateState};
    use miniz_oxide::{DataFormat, MZFlush};
    // Single stored block that is not end block.
    let enc = vec![0x78, 0x9c, 0x12, 0x34, 0x56];

    let mut inflate_stream = InflateState::new(DataFormat::Zlib);
    let mut output = vec![0u8; 30];

    let _ = inflate(
        &mut inflate_stream,
        enc.as_slice(),
        &mut output,
        MZFlush::None,
    );

    let inflate_result = inflate(&mut inflate_stream, &[], &mut output, MZFlush::Finish);

    assert_eq!(inflate_result.status.unwrap_err(), MZError::Buf);

    //let res = decompress_to_vec_zlib(enc.as_slice()).unwrap();
    //assert!(res.is_empty());
}

fn decode_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>()
}

#[test]
fn issue_161_index_out_of_range_apply_match() {
    // This data contains an match that has a distance before the start of the data.
    // and resulted in an edge cause causing a panic instead of returning with an error when using.
    // a smaller wrapping buffer.
    let content_hex = "fa99fff4f37fef5bbff9bb6ccb9ab4e47f66d9875cebf9ffe6eb6fbdf6e24b773f72ebe5175f62ff26bf78eec57bafdd78ee6b5f7efeee2b2f5b1d2bfe5100";
    let content = decode_hex(&content_hex);

    let mut decompressor = miniz_oxide::inflate::core::DecompressorOxide::new();

    let mut buf2 = vec![0; 2048];
    let _ = miniz_oxide::inflate::core::decompress(&mut decompressor, &content, &mut buf2, 0, 0);
}

#[test]
fn empty_stored() {
    // Compress empty input using stored compression level
    // There was a logic error casuing this to output zeroes
    // from the empty data buffer instead of outputting an empty stored block.
    let data = vec![];
    let enc = compress_to_vec_zlib(&data, 0);
    let _ = decompress_to_vec_zlib(&enc).unwrap();
}

#[test]
fn write_len_bytes_to_end() {
    use miniz_oxide::inflate::core;
    // Crashed due to overflow from condition being run in core::transfer due to accidentally using | instead of ||
    // after updating it, found by fuzzer.
    let data = get_test_file_data("tests/test_data/write_len_bytes_to_end");
    // Invalid deflate stream but we only care about the overflow.
    let _ = decompress_to_vec(&data);

    // Check also using wrapping buffer
    let mut buf2 = vec![0; 2];
    let mut decompressor = miniz_oxide::inflate::core::DecompressorOxide::new();
    let _ = miniz_oxide::inflate::core::decompress(
        &mut decompressor,
        &data,
        &mut buf2,
        0,
        core::inflate_flags::TINFL_FLAG_HAS_MORE_INPUT,
    );
}

/*
#[test]
fn partial_decompression_imap_issue_158() {
    use miniz_oxide::inflate::stream::{inflate, InflateState};
    use miniz_oxide::{DataFormat, MZFlush};
    use std::string;

    // Decompresses to
    // "* QUOTAROOT INBOX \"User quota\"\r\n* QUOTA \"User quota\" (STORAGE 76 307200)\r\nA0001 OK Getquotaroot completed (0.001 + 0.000 secs).\r\n"
    let input = vec![
        210, 82, 8, 12, 245, 15, 113, 12, 242, 247, 15, 81, 240, 244, 115, 242, 143, 80, 80, 10,
        45, 78, 45, 82, 40, 44, 205, 47, 73, 84, 226, 229, 210, 130, 200, 163, 136, 42, 104, 4,
        135, 248, 7, 57, 186, 187, 42, 152, 155, 41, 24, 27, 152, 27, 25, 24, 104, 242, 114, 57,
        26, 24, 24, 24, 42, 248, 123, 43, 184, 167, 150, 128, 213, 21, 229, 231, 151, 40, 36, 231,
        231, 22, 228, 164, 150, 164, 166, 40, 104, 24, 232, 129, 20, 104, 43, 128, 104, 3, 133,
        226, 212, 228, 98, 77, 61, 94, 46, 0, 0, 0, 0, 255, 255,
    ];

    let mut inflate_stream = InflateState::new(DataFormat::Raw);
    let mut output = vec![0; 8];
    let result = inflate(&mut inflate_stream, &input, &mut output, MZFlush::None);

    let out_string: String = string::String::from_utf8(output).unwrap();

    println!("{}", out_string);
    println!("written {}", result.bytes_written);

    assert!(result.status.is_ok());
    // Should not consume everything, there is not enough space in the buffer for the output.
    assert!(
        result.bytes_consumed < input.len(),
        "bytes consumed {:?}, input.len() {}",
        result.bytes_consumed,
        input.len()
    )
}*/

/*
#[test]
fn large_file() {
    let data = get_test_file_data("large_file/lf");
    let enc = compress_to_vec(&data.as_slice()[..], 3);

    let dec = decompress_to_vec(enc.as_slice()).unwrap();
    assert!(data == dec);
}

*/

// Test the behavior of TINFL_FLAG_STOP_ON_block-boundary, block-boundary_state(),
// and restarting the DecompressorOxide at a boundary.
#[test]
#[cfg(feature = "block-boundary")]
fn block_boundary() {
    for zlib in [false, true] {
        for restart in [false, true] {
            block_boundary_inner(zlib, restart);
        }
    }
}

#[cfg(feature = "block-boundary")]
fn block_boundary_inner(zlib: bool, restart: bool) {
    use std::collections::HashSet;
    // Compress some chunks of arbitrary data, ending each chunk with Sync (forcing a block boundary)

    // Large enough to trigger block boundaries in the middle of a chunk, so we're testing
    // more than just the Sync boundaries
    const CHUNK_SIZE: usize = 1024 * 96;
    const NUM_CHUNKS: usize = 8;

    let mut input = Vec::new();
    let mut compressed = Vec::new();
    let mut sync_points = HashSet::new();

    {
        use miniz_oxide::deflate::core::{self, TDEFLFlush, TDEFLStatus};

        let mut state = core::CompressorOxide::new(core::create_comp_flags_from_zip_params(
            1,
            if zlib { 15 } else { -15 },
            0,
        ));

        let mut buf = vec![0; CHUNK_SIZE * 2]; // compressed chunk
        let mut in_pos = 0; // total bytes of input compressed so far
        let mut rng = Rng::new(12345678);

        for i in 0..NUM_CHUNKS {
            // Generate a mix of incompressible and compressible input data
            let chunk = if i % 2 == 0 {
                rng.bytes(CHUNK_SIZE)
            } else {
                vec![0; CHUNK_SIZE]
            };
            input.extend_from_slice(&chunk);

            let (status, in_read, out_written) =
                core::compress(&mut state, &chunk, &mut buf, TDEFLFlush::Sync);
            assert_eq!(status, TDEFLStatus::Okay);
            assert_eq!(in_read, chunk.len());

            in_pos += in_read;
            sync_points.insert(in_pos);
            compressed.extend_from_slice(&buf[..out_written]);
        }

        // Finish compression
        let (status, in_read, out_written) =
            core::compress(&mut state, &[], &mut buf, TDEFLFlush::Finish);
        assert_eq!(status, TDEFLStatus::Done);
        assert_eq!(in_read, 0);
        compressed.extend_from_slice(&buf[..out_written]);
    }

    let mut block_boundaries = HashSet::new();

    {
        use miniz_oxide::inflate::core::{self, inflate_flags};
        use miniz_oxide::inflate::TINFLStatus;

        let mut state = core::DecompressorOxide::new();

        let mut out = vec![0; NUM_CHUNKS * CHUNK_SIZE];
        let mut out_pos = 0;
        let mut in_pos = 0;
        loop {
            let flags = if zlib {
                inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
            } else {
                0
            };

            let flags = flags
                | inflate_flags::TINFL_FLAG_STOP_ON_BLOCK_BOUNDARY
                | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

            let (status, in_read, out_written) =
                core::decompress(&mut state, &compressed[in_pos..], &mut out, out_pos, flags);
            in_pos += in_read;
            out_pos += out_written;

            if status == TINFLStatus::Done {
                break;
            }

            assert_eq!(status, TINFLStatus::BlockBoundary);
            block_boundaries.insert(out_pos);

            let bbs = state.block_boundary_state().unwrap();

            assert!(bbs.num_bits < 8);
            assert!(bbs.bit_buf >> bbs.num_bits == 0, "MSBs must be 0");

            if restart {
                let bbs = if !zlib {
                    // In non-Zlib mode, all the other fields are documented as redundant,
                    // so reset them to default
                    core::BlockBoundaryState {
                        num_bits: bbs.num_bits,
                        bit_buf: bbs.bit_buf,
                        ..Default::default()
                    }
                } else {
                    bbs
                };

                state = core::DecompressorOxide::from_block_boundary_state(&bbs);
            }
        }

        assert!(input == out, "decompressed output must match the input");
    }

    assert_eq!(
        sync_points.difference(&block_boundaries).count(),
        0,
        "every Sync must have a corresponding BlockBoundary"
    );
}

#[test]
fn change_compression_level_after_start() {
    use miniz_oxide::deflate::core::{self, TDEFLFlush, TDEFLStatus};
    use miniz_oxide::deflate::CompressionLevel;
    use miniz_oxide::inflate::decompress_to_vec;

    let mut compressed = Vec::new();
    let data = b"data 1 data 2"; //get_test_file_data("tests/test_data/numbers.txt");

    {
        let mut state =
            core::CompressorOxide::new(core::create_comp_flags_from_zip_params(0, 0, 0));

        let mut buf = vec![0; data.len()]; // compressed chunk

        let split_point = data.len() / 2;

        let (status, in_read, out_written) =
            core::compress(&mut state, &data[..split_point], &mut buf, TDEFLFlush::None);
        assert_eq!(status, TDEFLStatus::Okay);

        compressed.extend_from_slice(&buf[..out_written]);

        state.set_compression_level(CompressionLevel::BestCompression);

        // Finish compression
        let (status, _in_read, out_written) =
            core::compress(&mut state, &data[in_read..], &mut buf, TDEFLFlush::Finish);
        // Finish compression
        compressed.extend_from_slice(&buf[..out_written]);

        if status != TDEFLStatus::Done {
            let (status, _in_read, _out_written) =
                core::compress(&mut state, &[], &mut buf, TDEFLFlush::Finish);
            assert_eq!(status, TDEFLStatus::Done);
        }

        compressed.extend_from_slice(&buf[..out_written]);
    }

    let decomp = decompress_to_vec(&compressed).unwrap();
    assert_eq!(data[..], decomp);
}
