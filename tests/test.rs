extern crate miniz_oxide;
extern crate miniz_oxide_c_api;

use std::io::Read;

use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec;

fn get_test_file_data(name: &str) -> Vec<u8> {
    use std::fs::File;
    let mut input = Vec::new();
    let mut f = File::open(name).unwrap();

    f.read_to_end(&mut input).unwrap();
    input
}

fn get_test_data() -> Vec<u8> {
    use std::env;
    let path = env::var("TEST_FILE").unwrap_or_else(|_| "miniz/miniz.c".to_string());
    get_test_file_data(&path)
}

#[test]
fn roundtrip() {
    let level = 9;
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
fn roundtrip_level_1() {
    let level = 1;
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

/// Roundtrip test using the C API.
#[test]
fn c_api() {
    use miniz_oxide::{MZError, MZStatus};
    use miniz_oxide_c_api::{
        mz_deflate, mz_deflateEnd, mz_deflateInit, mz_deflateReset, mz_inflate, mz_inflateEnd,
        mz_inflateInit, mz_stream,
    };
    let mut data = get_test_data();
    let mut compressed = vec![0; data.len() + 10];
    let compressed_size;
    let decompressed_size;
    unsafe {
        let mut stream = mz_stream {
            next_in: data.as_mut_ptr(),
            avail_in: data.len() as u32,
            next_out: compressed.as_mut_ptr(),
            avail_out: compressed.len() as u32,
            ..Default::default()
        };

        assert_eq!(mz_deflateInit(&mut stream, 1), MZStatus::Ok as i32);
        assert_eq!(mz_deflate(&mut stream, 4), MZStatus::StreamEnd as i32);
        compressed_size = stream.total_out;
        assert_eq!(data.len() as u64, stream.total_in);

        // Check that reseting works properly.
        let mut compressed_2 = vec![0; compressed_size as usize];
        assert_eq!(mz_deflateReset(&mut stream), MZStatus::Ok as i32);
        stream.next_in = data.as_mut_ptr();
        stream.avail_in = data.len() as u32;
        stream.next_out = compressed_2.as_mut_ptr();
        stream.avail_out = compressed_2.len() as u32;
        assert_eq!(mz_deflate(&mut stream, 4), MZStatus::StreamEnd as i32);

        // This should fail, trying to decompress with a compressor.
        assert_eq!(mz_inflate(&mut stream, 4), MZError::Param as i32);
        assert_eq!(mz_inflateEnd(&mut stream), MZError::Param as i32);

        assert_eq!(mz_deflateEnd(&mut stream), MZStatus::Ok as i32);

        assert_eq!(compressed_size, stream.total_out);
        assert!(compressed[..compressed_size as usize] == compressed_2[..]);
    }

    assert!(compressed_size as usize <= compressed.len());

    let mut decompressed = vec![0; data.len()];

    unsafe {
        let mut stream = mz_stream {
            next_in: compressed.as_mut_ptr(),
            avail_in: compressed_size as u32,
            next_out: decompressed.as_mut_ptr(),
            avail_out: decompressed.len() as u32,
            ..Default::default()
        };

        assert_eq!(mz_inflateInit(&mut stream), MZStatus::Ok as i32);
        assert_eq!(mz_inflate(&mut stream, 4), MZStatus::StreamEnd as i32);
        assert_eq!(mz_inflateEnd(&mut stream), MZStatus::Ok as i32);

        decompressed_size = stream.total_out;
        assert_eq!(compressed_size, stream.total_in);

        // This should fail as the stream is an inflate stream!
        assert_eq!(mz_deflate(&mut stream, 4), MZError::Param as i32);
        assert_eq!(mz_deflateEnd(&mut stream), MZError::Param as i32);
    }

    assert_eq!(data[..], decompressed[0..decompressed_size as usize]);
}
