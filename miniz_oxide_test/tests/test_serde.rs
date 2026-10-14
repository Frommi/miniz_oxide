use miniz_oxide::{
    deflate::compress_to_vec,
    inflate::{
        core::{decompress, inflate_flags, BlockBoundaryState, DecompressorOxide},
        decompress_to_vec, TINFLStatus,
    },
};

#[test]
fn serde_serializes_full_inflate_state() {
    assert!(rmp_serde::to_vec(&DecompressorOxide::default()).is_ok());
}

/// Test pause and resume of decompression at a block boundary.
#[test]
fn serde_resume_inflate_state() {
    let first_block: &[u8] = b"first block";
    let second_block: &[u8] = b"second block";
    let data = stored_blocks(first_block, second_block);
    let expected = [first_block, second_block].concat();

    let mut decomp = Box::<DecompressorOxide>::default();
    let mut out_buf = vec![0; expected.len()];
    let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
        | inflate_flags::TINFL_FLAG_STOP_ON_BLOCK_BOUNDARY;
    let (status, in_pos, out_pos) = decompress(&mut decomp, &data, &mut out_buf, 0, flags);
    assert_eq!(status, TINFLStatus::BlockBoundary);

    let state = decomp.block_boundary_state().unwrap();

    // here the 'save' and 'restore' happens
    let (in_pos, state) = serde_serialize_deserialize_state((in_pos, state));
    let mut decomp = DecompressorOxide::from_block_boundary_state(&state);
    let (status, in_consumed, out_consumed) = decompress(
        &mut decomp,
        &data[in_pos..],
        &mut out_buf,
        out_pos,
        inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );

    assert_eq!(status, TINFLStatus::Done);
    assert_eq!(in_pos + in_consumed, data.len());
    assert_eq!(out_pos + out_consumed, expected.len());
    assert_eq!(out_buf, expected);
}

#[test]
fn serde_rejects_invalid_block_boundary_state() {
    let invalid_num_bits = rmp_serde::to_vec(&(8_u8, 0_u8, 0_u32, 0_u32, 1_u32)).unwrap();
    assert!(rmp_serde::from_slice::<BlockBoundaryState>(&invalid_num_bits).is_err());

    let invalid_bit_buf = rmp_serde::to_vec(&(1_u8, 2_u8, 0_u32, 0_u32, 1_u32)).unwrap();
    assert!(rmp_serde::from_slice::<BlockBoundaryState>(&invalid_bit_buf).is_err());
}

fn stored_blocks(first: &[u8], second: &[u8]) -> Vec<u8> {
    let mut compressed = Vec::new();
    for (header, block) in [(0x00, first), (0x01, second)] {
        let len = u16::try_from(block.len()).unwrap();
        compressed.push(header);
        compressed.extend_from_slice(&len.to_le_bytes());
        compressed.extend_from_slice(&(!len).to_le_bytes());
        compressed.extend_from_slice(block);
    }
    compressed
}

/// Saves the state and 'resumes' it
pub fn serde_serialize_deserialize_state(
    state: (usize, BlockBoundaryState),
) -> (usize, BlockBoundaryState) {
    let decompressor_state_msgpack = rmp_serde::to_vec(&state).unwrap();
    let decompressor_state_compressed = compress_to_vec(&decompressor_state_msgpack, 7);

    dbg!(decompressor_state_msgpack.len());
    dbg!(decompressor_state_compressed.len());

    let decompressor_state_msgpack = decompress_to_vec(&decompressor_state_compressed).unwrap();
    rmp_serde::from_slice(&decompressor_state_msgpack).unwrap()
}
