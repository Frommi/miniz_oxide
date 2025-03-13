use miniz_oxide::{
    deflate::compress_to_vec,
    inflate::{
        core::{decompress, inflate_flags, DecompressorOxide},
        decompress_to_vec, TINFLStatus,
    },
};

/// Test pause and resume of DecompressorOxide state
#[test]
fn serde_resume_inflate_state() {
    let data = include_bytes!("../../miniz_oxide/tests/test_data/numbers.deflate");
    let decompressed_fully = decompress_to_vec(data.as_slice()).unwrap();

    let (decomp, in_pos, out_buf) = {
        let decomp = Box::<DecompressorOxide>::default();
        let out_buf = Vec::new();
        let result = decompress_to_vec_partial(decomp, &data[..], out_buf, 32 * 1024);

        match result {
            PartialResult::Ok(_) => panic!("expected partial read"),
            PartialResult::Err(err) => panic!("expected partial read, err: {err:?}"),

            PartialResult::Partial(in_pos, decomp, out_buf) => {
                println!("partial read len={}", out_buf.len());
                (decomp, in_pos, out_buf)
            }
        }
    };
    println!("save at in_pos={in_pos}");

    // here the 'save' and 'restore' happens
    let (in_pos, decomp) = serde_serialize_deserialize_decompressor((in_pos, decomp));

    println!("resume at in_pos={in_pos}");

    let result = decompress_to_vec_partial(decomp, &data[in_pos..], out_buf, 1_000_000);

    let out_buf = match result {
        PartialResult::Partial(_, _, _) => panic!("expected full read"),
        PartialResult::Err(err) => panic!("expected full read, err: {err:?}"),

        PartialResult::Ok(out_buf) => out_buf,
    };

    assert_eq!(out_buf, decompressed_fully);
}

/// Saves the state and 'resumes' it
pub fn serde_serialize_deserialize_decompressor(
    decomp: (usize, Box<DecompressorOxide>),
) -> (usize, Box<DecompressorOxide>) {
    let decompressor_state_msgpack = rmp_serde::to_vec(&decomp).unwrap();
    let decompressor_state_compressed = compress_to_vec(&decompressor_state_msgpack, 7);

    dbg!(decompressor_state_msgpack.len());
    dbg!(decompressor_state_compressed.len());

    let decompressor_state_msgpack = decompress_to_vec(&decompressor_state_compressed).unwrap();
    rmp_serde::from_slice(&decompressor_state_msgpack).unwrap()
}

#[derive(Clone)]
enum PartialResult<T, E> {
    /// out_buf
    Ok(T),

    /// input_pos, decomp, out_buf
    Partial(usize, Box<DecompressorOxide>, T),
    Err(E),
}

/// Decompressed partially to out_buf.
///
/// Assumes that out_buf contains already decompressed data.
fn decompress_to_vec_partial(
    decomp: Box<DecompressorOxide>,
    input: &[u8],
    out_buf: Vec<u8>,
    pause_output_size: usize,
) -> PartialResult<Vec<u8>, TINFLStatus> {
    decompress_to_vec_partial_inner(decomp, input, out_buf, 0, pause_output_size)
}

fn decompress_to_vec_partial_inner<'b>(
    mut decomp: Box<DecompressorOxide>,
    input: &[u8],
    mut out_buf: Vec<u8>,
    flags: u32,
    pause_output_size: usize,
) -> PartialResult<Vec<u8>, TINFLStatus> {
    let flags = flags | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let mut out_pos = out_buf.len();
    let mut in_pos = 0;

    let additional = out_buf.len().max(16);
    let new_size = out_buf
        .len()
        .saturating_add(additional)
        .min(pause_output_size);
    out_buf.resize(new_size, 0);

    loop {
        // Wrap the whole output slice so we know we have enough of the
        // decompressed data for matches.
        let (status, in_consumed, out_consumed) =
            decompress(&mut decomp, &input[in_pos..], &mut out_buf, out_pos, flags);
        out_pos += out_consumed;

        match status {
            TINFLStatus::Done => {
                out_buf.truncate(out_pos);
                return PartialResult::Ok(out_buf);
            }

            TINFLStatus::HasMoreOutput => {
                // in_consumed is not expected to be out of bounds,
                // but the check eliminates a panicking code path
                if in_consumed > input.len() {
                    return PartialResult::Err(TINFLStatus::HasMoreOutput);
                }
                in_pos += in_consumed;

                // if the buffer has already reached the size limit, return partial result
                if out_buf.len() >= pause_output_size {
                    out_buf.truncate(out_pos);
                    return PartialResult::Partial(in_pos, decomp, out_buf);
                }
                // calculate the new length, capped at `pause_output_size`
                let new_len = out_buf.len().saturating_mul(2).min(pause_output_size);
                out_buf.resize(new_len, 0);
            }

            _ => return PartialResult::Err(status),
        }
    }
}
