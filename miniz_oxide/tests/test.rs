extern crate miniz_oxide;

use std::io::Read;

use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};
use miniz_oxide::inflate::{decompress_to_vec, decompress_to_vec_zlib, TINFLStatus};

fn get_test_file_data(name: &str) -> Vec<u8> {
    use std::fs::File;
    let mut input = Vec::new();
    let mut f = File::open(name).unwrap();

    f.read_to_end(&mut input).unwrap();
    input
}

/// Fuzzed file that caused issues for the inflate library.
#[test]
fn inf_issue_14() {
    let data = get_test_file_data("tests/test_data/issue_14.zlib");
    let result = decompress_to_vec_zlib(data.as_slice());
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error, TINFLStatus::Failed);
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
    let enc = compress_to_vec(&data.as_slice()[..], level);
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
    use std::io::Cursor;

    let input = get_test_file_data("tests/test_data/numbers.deflate");
    let data = get_test_file_data("tests/test_data/numbers.txt");

    let decomp = |input: &[u8]| {
        let mut decomp = core::DecompressorOxide::new();
        decomp.init();

        let mut output = [0; core::TINFL_LZ_DICT_SIZE];
        let mut output_cursor = Cursor::new(&mut output[..]);
        let flags = core::inflate_flags::TINFL_FLAG_HAS_MORE_INPUT;

        let (status, in_consumed, out_consumed) =
            core::decompress(&mut decomp, input, &mut output_cursor, flags);

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
