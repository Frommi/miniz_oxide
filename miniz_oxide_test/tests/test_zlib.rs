//! Tests that test behaviour against normal zlib/flate2.
//!
//! Currently only testing for encoding with reduced window bits as failing on decoding it is not implemented in miniz_oxide.

use miniz_oxide::deflate::core::{CompressionStrategy, CompressorOxide};
use std::io::Read;

fn generate_input() -> Vec<u8> {
    let vec_len = 1usize << 16;
    let mut input_bytes = Vec::with_capacity(vec_len);

    // Generate pseudo-random bytes just to get to the right length w/o introducing other patterns or dependencies,
    // and keep things deterministic
    let mut x: u32 = 0x9E3779B9;
    for _ in 0..vec_len {
        x = x.wrapping_mul(0xDEADBEEF);
        let next_byte = (x >> 24) as u8;
        input_bytes.push(next_byte);
    }

    // Force some data to repeat every 16384 bytes, to bait the encoder
    let repeat_bytes = input_bytes[..64].to_vec();
    for ii in 1..4 {
        let byte_idx = 16384 * ii;
        input_bytes[byte_idx..][..64].copy_from_slice(&repeat_bytes);
    }

    input_bytes
}

fn compress_miniz(
    input: &[u8],
    level: u8,
    window_bits: u8,
    strategy: CompressionStrategy,
) -> Vec<u8> {
    let mut comp =
        CompressorOxide::with_params(miniz_oxide::DataFormat::Zlib, level, strategy, window_bits);

    let mut out = vec![0u8; input.len() * 2 + 1024];

    let (status, _in_pos, out_pos) = miniz_oxide::deflate::core::compress(
        &mut comp,
        input,
        &mut out,
        miniz_oxide::deflate::core::TDEFLFlush::Finish,
    );

    // Should have enough output space
    assert_eq!(status, miniz_oxide::deflate::core::TDEFLStatus::Done);

    // trim excess
    out.truncate(out_pos);

    out
}

fn decode_with_zlib(bytes: &[u8], window_bits: u8) -> Vec<u8> {
    let mut z = flate2::read::ZlibDecoder::new_with_decompress(
        bytes,
        flate2::Decompress::new_with_window_bits(true, window_bits),
    );
    let mut out_buff = Vec::new();
    z.read_to_end(&mut out_buff)
        .expect("failed to decode w/ zlib");
    out_buff
}

fn test_window_bits(level: u8, strategy: CompressionStrategy, input: &[u8]) {
    for window_bits in [15, 14, 13, 12, 11, 10, 9] {
        println!("Testing window bits {}", window_bits);
        let encoded_by_miniz = compress_miniz(&input, level, window_bits, strategy);

        println!("First byte of compressed: {:#02X}", encoded_by_miniz[0]);

        let decoded_by_miniz =
            miniz_oxide::inflate::decompress_to_vec_zlib(&encoded_by_miniz).unwrap();
        assert_eq!(decoded_by_miniz, input);

        println!("Decoded by miniz okay");

        let decoded_by_zlib = decode_with_zlib(&encoded_by_miniz, window_bits);
        assert_eq!(decoded_by_zlib, input);

        println!("Decoded by flate2/zlib okay\n");
    }
}

#[test]
// Test that compressing with a lower number than window bits than normal works correctly.
// TODO: need to create proper data that would verifiably trip up different encoder window bit sizes,
// This may just test a few right now.
fn window_bits() {
    let input = generate_input();

    for level in [1, 6, 8] {
        test_window_bits(level, CompressionStrategy::Default, &input);
    }

    // Make sure this works as intended.
    test_window_bits(6, CompressionStrategy::RLE, &input);
}
