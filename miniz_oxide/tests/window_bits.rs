//! Regression test for https://github.com/Frommi/miniz_oxide/issues/215
//!
//! Compressing with a reduced `window_bits` (e.g. 13) must never emit a back-reference
//! (length/distance) whose distance exceeds the advertised window size. The zlib header
//! advertises the window size, so a distance beyond it produces a stream that `zlib`
//! rejects as corrupt even though miniz's own inflate accepts it.
#![cfg(feature = "with-alloc")]

use miniz_oxide::deflate::core::{compress, CompressorOxide, CompressionStrategy, TDEFLFlush, TDEFLStatus};
use miniz_oxide::DataFormat;

/// The issue's reproducer: pseudo-random 64 KiB with a 64-byte block repeated every 16384
/// bytes, which baits the encoder into a long-distance match.
fn generate_input() -> Vec<u8> {
    let vec_len = 1usize << 16;
    let mut input_bytes = Vec::with_capacity(vec_len);

    let mut x: u32 = 0x9E3779B9;
    for _ in 0..vec_len {
        x = x.wrapping_mul(0xDEADBEEF);
        input_bytes.push((x >> 24) as u8);
    }

    let repeat_bytes = input_bytes[..64].to_vec();
    for ii in 1..4 {
        let byte_idx = 16384 * ii;
        input_bytes[byte_idx..][..64].copy_from_slice(&repeat_bytes);
    }

    input_bytes
}

fn compress_miniz(input: &[u8], level: u8, window_bits: u8) -> Vec<u8> {
    let mut comp = CompressorOxide::with_params(
        DataFormat::Zlib,
        level,
        CompressionStrategy::Default,
        window_bits,
    );

    let mut out = vec![0u8; input.len() * 2 + 1024];
    let (status, _in_pos, out_pos) = compress(&mut comp, input, &mut out, TDEFLFlush::Finish);
    assert_eq!(status, TDEFLStatus::Done);

    out.truncate(out_pos);
    out
}

// --- Minimal DEFLATE bit reader that walks the compressed block and records every
// length/distance pair actually present in the bitstream. ---

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize, // bit position
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn bits(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for i in 0..n {
            let byte = self.data[self.pos >> 3];
            let bit = (byte >> (self.pos & 7)) & 1;
            v |= (bit as u32) << i;
            self.pos += 1;
        }
        v
    }
}

fn decode_symbol(r: &mut BitReader, lens: &[u8], codes: &[u32]) -> usize {
    let mut code = 0u32;
    for len in 1..=15u32 {
        code |= r.bits(1) << (len - 1);
        // codes are stored in MSB-first bit-reversed order for deflate
        for (sym, (&l, &c)) in lens.iter().zip(codes.iter()).enumerate() {
            if l as u32 == len && c == code {
                return sym;
            }
        }
    }
    panic!("invalid Huffman code");
}

fn rev(code: u32, len: u32) -> u32 {
    let mut r = 0;
    for i in 0..len {
        r |= ((code >> i) & 1) << (len - 1 - i);
    }
    r
}

/// Build canonical Huffman decode tables (lens -> bit-reversed codes) per RFC 1951.
fn canonical(lens: &[u8]) -> Vec<u32> {
    // count codes per length
    let mut bl_count = [0u32; 16];
    for &l in lens {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }
    let mut next_code = [0u32; 16];
    let mut code = 0u32;
    for bits in 1..=15u32 {
        code = (code + bl_count[bits as usize - 1]) << 1;
        next_code[bits as usize] = code;
    }
    let mut out = vec![0u32; lens.len()];
    for (sym, &l) in lens.iter().enumerate() {
        if l > 0 {
            let c = next_code[l as usize];
            next_code[l as usize] += 1;
            out[sym] = rev(c, l as u32);
        }
    }
    out
}

/// Parse a raw (header-stripped) DEFLATE stream and return the maximum back-reference
/// distance present. Panics on anything unexpected.
fn max_backref_distance(raw: &[u8]) -> u32 {
    let mut r = BitReader::new(raw);
    let mut max_dist = 0u32;

    loop {
        let bfinal = r.bits(1);
        let btype = r.bits(2);
        if btype == 0 {
            // Stored block: align to byte, read LEN/NLEN, skip payload.
            r.pos = (r.pos + 7) & !7;
            let len = r.bits(16) as usize;
            let _nlen = r.bits(16);
            r.pos += len * 8;
            if bfinal == 1 {
                break;
            }
            continue;
        }
        assert_eq!(btype, 2, "only dynamic/stored blocks expected from the encoder");

        // Dynamic block header
        let hlit = r.bits(5) + 257;
        let hdist = r.bits(5) + 1;
        let hclen = r.bits(4) + 4;

        const ORDER: [usize; 19] = [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
        ];
        let mut cl_lens = [0u8; 19];
        for &o in ORDER.iter().take(hclen as usize) {
            cl_lens[o] = r.bits(3) as u8;
        }
        let cl_codes = canonical(&cl_lens);

        // Decode the literal/length and distance code lengths.
        let total = (hlit + hdist) as usize;
        let mut lens: Vec<u8> = Vec::with_capacity(total);
        while lens.len() < total {
            let sym = decode_symbol(&mut r, &cl_lens, &cl_codes);
            match sym {
                0..=15 => lens.push(sym as u8),
                16 => {
                    let prev = *lens.last().unwrap();
                    let rep = 3 + r.bits(2);
                    lens.extend(std::iter::repeat(prev).take(rep as usize));
                }
                17 => {
                    let rep = 3 + r.bits(3);
                    lens.resize(lens.len() + rep as usize, 0);
                }
                18 => {
                    let rep = 11 + r.bits(7);
                    lens.resize(lens.len() + rep as usize, 0);
                }
                _ => panic!("bad code-length symbol {sym}"),
            }
        }

        let lit_lens = &lens[..hlit as usize];
        let dist_lens = &lens[hlit as usize..];
        let lit_codes = canonical(lit_lens);
        let dist_codes = canonical(dist_lens);

        // Decode symbols.
        loop {
            let sym = decode_symbol(&mut r, lit_lens, &lit_codes);
            match sym {
                0..=255 => {} // literal
                256 => break, // end of block
                257..=285 => {
                    // length
                    const LEN_EXTRA: [u32; 29] = [
                        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5,
                        5, 5, 5, 0,
                    ];
                    let idx = sym - 257;
                    let _len = r.bits(LEN_EXTRA[idx]);

                    // distance
                    let dsym = decode_symbol(&mut r, dist_lens, &dist_codes);
                    const DIST_EXTRA: [u32; 30] = [
                        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10,
                        11, 11, 12, 12, 13, 13,
                    ];
                    const DIST_BASE: [u32; 30] = [
                        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513,
                        769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
                    ];
                    let d = DIST_BASE[dsym] + r.bits(DIST_EXTRA[dsym]);
                    if d > max_dist {
                        max_dist = d;
                    }

                    // Stop early once we've exceeded any sane window - keeps the wrapper bounded.
                    if max_dist > 32_768 {
                        return max_dist;
                    }
                }
                _ => panic!("bad literal/length symbol {sym}"),
            }
        }

        if bfinal == 1 {
            break;
        }
    }

    max_dist
}

#[test]
fn window_bits_13_does_not_emit_backrefs_beyond_window() {
    let input = generate_input();

    for window_bits in [15u8, 14, 13] {
        let encoded = compress_miniz(&input, 1, window_bits);
        // Strip the 2-byte zlib header (CMF+FLG); the rest is the raw DEFLATE stream.
        let raw = &encoded[2..encoded.len() - 4]; // minus 4-byte adler32

        let max_dist = max_backref_distance(raw);
        let window_size = 1u32 << window_bits;

        // Sanity: round-trips through miniz.
        let decoded = miniz_oxide::inflate::decompress_to_vec_zlib(&encoded).unwrap();
        assert_eq!(decoded, input, "window_bits={window_bits} must round-trip");

        assert!(
            max_dist <= window_size,
            "window_bits={window_bits}: emitted back-reference distance {max_dist} \
             exceeds window size {window_size} (issue #215)",
        );
    }
}
