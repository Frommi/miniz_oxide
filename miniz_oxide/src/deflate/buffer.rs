//! Buffer wrappers implementing default so we can allocate the buffers with `Box::default()`
//! to avoid stack copies. Box::new() doesn't at the moment, and using a vec means we would lose
//! static length info.

use crate::deflate::core::{LZ_DICT_SIZE, MAX_MATCH_LEN};
use alloc::boxed::Box;
use alloc::vec;

/// Size of the buffer of lz77 encoded data.
pub const LZ_CODE_BUF_SIZE: usize = 64 * 1024;
pub const LZ_CODE_BUF_MASK: usize = LZ_CODE_BUF_SIZE - 1;
/// Size of the output buffer.
pub const OUT_BUF_SIZE: usize = (LZ_CODE_BUF_SIZE * 13) / 10;
pub const LZ_DICT_FULL_SIZE: usize = LZ_DICT_SIZE + MAX_MATCH_LEN - 1 + 1;

/// Size of hash values in the hash chains and in the table of 3-byte sequences.
pub const LZ_HASH_BITS: u32 = 14;
/// Number of entries in each of the two halves of `HashBuffers::hash`.
pub const LZ_HASH_SIZE: usize = 1 << LZ_HASH_BITS;

/// `bits`-bit hash of `bytes`; callers mask off the bytes that are not part of the key.
#[inline]
pub const fn hash(bytes: u32, bits: u32) -> usize {
    (bytes.wrapping_mul(0x9E37_79B1) >> (32 - bits)) as usize
}

#[derive(Clone)]
pub struct HashBuffers {
    pub dict: Box<[u8; LZ_DICT_FULL_SIZE]>,
    pub next: Box<[u16; LZ_DICT_SIZE]>,
    /// The heads of the 4-byte hash chains, followed by the most recent position of each 3-byte
    /// hash. `compress_fast` uses the start as its own hash table.
    pub hash: Box<[u16; 2 * LZ_HASH_SIZE]>,
}

impl HashBuffers {
    #[inline]
    pub fn reset(&mut self) {
        self.dict.fill(0);
        self.next.fill(0);
        self.hash.fill(0);
    }
}

impl Default for HashBuffers {
    fn default() -> HashBuffers {
        HashBuffers {
            dict: vec![0; LZ_DICT_FULL_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            next: vec![0; LZ_DICT_SIZE].into_boxed_slice().try_into().unwrap(),
            hash: vec![0; 2 * LZ_HASH_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
        }
    }
}

#[derive(Clone)]
pub struct LocalBuf {
    pub b: [u8; OUT_BUF_SIZE],
}

impl Default for LocalBuf {
    fn default() -> LocalBuf {
        LocalBuf {
            b: [0; OUT_BUF_SIZE],
        }
    }
}
