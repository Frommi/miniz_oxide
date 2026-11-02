#![cfg(feature = "with-alloc")]

use miniz_oxide::deflate::core::{
    compress, compress_to_output, CompressorOxide, TDEFLFlush, TDEFLStatus,
};
use miniz_oxide::deflate::stream::deflate;
use miniz_oxide::inflate::stream::{inflate, InflateState};
use miniz_oxide::inflate::{decompress_to_vec, decompress_to_vec_zlib};
use miniz_oxide::{DataFormat, MZFlush, MZStatus};

fn incompressible(len: usize) -> Vec<u8> {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 56) as u8
        })
        .collect()
}

fn compressor(format: DataFormat, level: u8) -> CompressorOxide {
    let mut compressor = CompressorOxide::default();
    compressor.set_format_and_level(format, level);
    compressor
}

// Follow the zlib contract: a flush is complete when a call leaves output space.
// In particular, do not make an extra empty-input call after this point.
fn flush_stream(
    compressor: &mut CompressorOxide,
    mut input: &[u8],
    output_size: usize,
    flush: MZFlush,
) -> Vec<u8> {
    let mut compressed = Vec::new();
    let mut output = vec![0; output_size];
    let call_limit = input.len() * 2 + 100;
    for _ in 0..call_limit {
        let result = deflate(compressor, input, &mut output, flush);
        assert!(result.status.is_ok(), "{result:?}");
        compressed.extend_from_slice(&output[..result.bytes_written]);
        input = &input[result.bytes_consumed..];
        if result.bytes_written < output.len() || result.status == Ok(MZStatus::StreamEnd) {
            assert!(input.is_empty());
            assert_eq!(
                result.status,
                Ok(if flush == MZFlush::Finish {
                    MZStatus::StreamEnd
                } else {
                    MZStatus::Ok
                })
            );
            return compressed;
        }
    }
    panic!("flush did not finish");
}

fn check_available(state: &mut InflateState, compressed: &[u8], expected: &[u8], finished: bool) {
    let mut decoded = vec![0; expected.len() + 1];
    let result = inflate(state, compressed, &mut decoded, MZFlush::None);
    assert_eq!(
        result.status,
        Ok(if finished {
            MZStatus::StreamEnd
        } else {
            MZStatus::Ok
        })
    );
    assert_eq!(result.bytes_consumed, compressed.len());
    assert_eq!(result.bytes_written, expected.len());
    assert_eq!(&decoded[..result.bytes_written], expected);
}

#[test]
fn sync_flush_finishes_when_output_space_remains() {
    // Regression for #210: the last drain of an automatic block used to return
    // 7174 bytes, leaving spare space while 12 bytes of the sync flush were pending.
    let data = incompressible(31_751);
    let compressed = flush_stream(
        &mut compressor(DataFormat::Raw, 6),
        &data,
        8192,
        MZFlush::Sync,
    );
    assert!(compressed.ends_with(&[0, 0, 0xff, 0xff]));
    check_available(
        &mut InflateState::new_boxed(DataFormat::Raw),
        &compressed,
        &data,
        false,
    );
}

#[test]
fn flushes_at_block_and_lookahead_boundaries() {
    let data = incompressible(65_537);
    for level in [0, 1, 6, 9] {
        for format in [DataFormat::Raw, DataFormat::Zlib] {
            for flush in [
                MZFlush::Partial,
                MZFlush::Sync,
                MZFlush::Full,
                MZFlush::Finish,
            ] {
                for len in [0, 257, 258, 31_744, 31_745, 31_751, 32_000, 65_537] {
                    for output_size in [7, 257, 8192, 32_768] {
                        let mut c = compressor(format, level);
                        let compressed = flush_stream(&mut c, &data[..len], output_size, flush);
                        check_available(
                            &mut InflateState::new_boxed(format),
                            &compressed,
                            &data[..len],
                            flush == MZFlush::Finish,
                        );
                        if matches!(flush, MZFlush::Sync | MZFlush::Full) {
                            assert!(compressed.ends_with(&[0, 0, 0xff, 0xff]));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn successive_records_are_available_before_finish() {
    let data = incompressible(31_751);
    for level in [0, 1, 6, 9] {
        for format in [DataFormat::Raw, DataFormat::Zlib] {
            let mut c = compressor(format, level);
            let mut state = InflateState::new_boxed(format);
            let mut compressed = Vec::new();
            let mut expected = Vec::new();
            for flush in [
                MZFlush::Sync,
                MZFlush::Partial,
                MZFlush::Full,
                MZFlush::Finish,
            ] {
                let record = flush_stream(&mut c, &data, 8192, flush);
                check_available(&mut state, &record, &data, flush == MZFlush::Finish);
                compressed.extend_from_slice(&record);
                expected.extend_from_slice(&data);
            }
            let decoded = if format == DataFormat::Zlib {
                // This also verifies the checksum accumulated across buffer drains.
                decompress_to_vec_zlib(&compressed).unwrap()
            } else {
                decompress_to_vec(&compressed).unwrap()
            };
            assert_eq!(decoded, expected);
        }
    }
}

#[test]
fn full_flush_resets_dictionary_after_buffer_drain() {
    let data = incompressible(31_751);
    for level in [0, 1, 6, 9] {
        let mut c = compressor(DataFormat::Raw, level);
        let mut compressed = flush_stream(&mut c, &data, 8192, MZFlush::Full);
        let suffix = flush_stream(&mut c, &data, 257, MZFlush::Finish);
        // Reusing the same record would encode backward references if Full had
        // returned before its marker and dictionary reset were processed.
        assert_eq!(decompress_to_vec(&suffix).unwrap(), data);
        compressed.extend_from_slice(&suffix);
        assert_eq!(decompress_to_vec(&compressed).unwrap(), data.repeat(2));
    }
}

fn callback_output(c: &mut CompressorOxide, input: &[u8], flush: TDEFLFlush) -> Vec<u8> {
    let mut output = Vec::new();
    let (status, consumed) = compress_to_output(c, input, flush, |bytes| {
        output.extend_from_slice(bytes);
        true
    });
    assert_eq!(consumed, input.len());
    assert_eq!(
        status,
        if flush == TDEFLFlush::Finish {
            TDEFLStatus::Done
        } else {
            TDEFLStatus::Okay
        }
    );
    output
}

#[test]
fn draining_completed_flush_does_not_repeat_marker() {
    let data = incompressible(512);
    for mode in [
        TDEFLFlush::Partial,
        TDEFLFlush::Sync,
        TDEFLFlush::Full,
        TDEFLFlush::Finish,
        TDEFLFlush::NoSync,
        TDEFLFlush::PartialOpt,
        TDEFLFlush::SyncOpt,
    ] {
        let expected = callback_output(&mut compressor(DataFormat::Zlib, 6), &data, mode);
        // These buffers all split an already generated explicit flush. Retrying
        // must drain that flush without appending a second marker (see #105).
        for first_size in [0, 1, 2, 6, expected.len() - 1] {
            let mut c = compressor(DataFormat::Zlib, 6);
            let mut first = vec![0; first_size];
            let (status, consumed, written) = compress(&mut c, &data, &mut first, mode);
            assert_eq!(status, TDEFLStatus::Okay);
            assert_eq!(consumed, data.len());
            assert_eq!(written, first.len());
            let mut rest = vec![0; expected.len() + 16];
            let (status, consumed, written) = compress(&mut c, &[], &mut rest, mode);
            assert_eq!(
                status,
                if mode == TDEFLFlush::Finish {
                    TDEFLStatus::Done
                } else {
                    TDEFLStatus::Okay
                }
            );
            assert_eq!(consumed, 0);
            first.extend_from_slice(&rest[..written]);
            assert_eq!(first, expected);
        }
    }
}

#[test]
fn new_flush_mode_after_pending_output() {
    let data = incompressible(512);
    let mut c = compressor(DataFormat::Raw, 6);
    let mut one = [0; 1];
    let (_, consumed, written) = compress(&mut c, &data, &mut one, TDEFLFlush::NoSync);
    assert_eq!(consumed, data.len());
    let mut compressed = one[..written].to_vec();
    // Change mode while the old block is still buffered, and drain it over
    // multiple calls. NoSync output must not be mistaken for a completed Sync.
    let (_, consumed, written) = compress(&mut c, &[], &mut one, TDEFLFlush::Sync);
    assert_eq!(consumed, 0);
    compressed.extend_from_slice(&one[..written]);
    let mut output = [0; 1024];
    let (status, consumed, written) = compress(&mut c, &[], &mut output, TDEFLFlush::Sync);
    assert_eq!(status, TDEFLStatus::Okay);
    assert_eq!(consumed, 0);
    compressed.extend_from_slice(&output[..written]);
    assert!(compressed.ends_with(&[0, 0, 0xff, 0xff]));
    check_available(
        &mut InflateState::new_boxed(DataFormat::Raw),
        &compressed,
        &data,
        false,
    );
}

#[test]
fn exact_output_boundary_and_repeated_empty_flushes() {
    let data = incompressible(512);
    let mut reference = compressor(DataFormat::Raw, 6);
    let expected = callback_output(&mut reference, &data, TDEFLFlush::Sync);
    let mut c = compressor(DataFormat::Raw, 6);
    let mut output = vec![0; expected.len()];
    let result = deflate(&mut c, &data, &mut output, MZFlush::Sync);
    assert_eq!(result.bytes_written, output.len());
    assert_eq!(output, expected);
    // An exactly full flush can be followed by another marker. Preserve that
    // documented behavior, and ensure empty calls terminate with enough space.
    for _ in 0..2 {
        let expected = callback_output(&mut reference, &[], TDEFLFlush::Sync);
        let marker = flush_stream(&mut c, &[], 7, MZFlush::Sync);
        assert_eq!(marker, expected);
        output.extend_from_slice(&marker);
    }
    let tail = flush_stream(&mut c, &[], 7, MZFlush::Finish);
    output.extend_from_slice(&tail);
    assert_eq!(decompress_to_vec(&output).unwrap(), data);
}
