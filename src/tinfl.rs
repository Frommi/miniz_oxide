#![allow(dead_code)]

use libc::*;
use miniz_oxide::inflate::core::DecompressorOxide;
// pub use miniz_oxide::inflate::core::DecompressorOxide as tinfl_decompressor;
pub use miniz_oxide::inflate::core::{decompress, inflate_flags};
use miniz_oxide::inflate::TINFLStatus;
use std::{ptr, slice, usize};

pub const TINFL_DECOMPRESS_MEM_TO_MEM_FAILED: size_t = usize::MAX;

#[allow(bad_style)]
#[repr(C)]
pub enum tinfl_status {
    /* This flags indicates the inflator needs 1 or more input bytes to make forward progress, but the caller is indicating that no more are available. The compressed data */
    /* is probably corrupted. If you call the inflator again with more bytes it'll try to continue processing the input but this is a BAD sign (either the data is corrupted or you called it incorrectly). */
    /* If you call it again with no input you'll just get TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS again. */
    TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS = -4,

    /* This flag indicates that one or more of the input parameters was obviously bogus. (You can try calling it again, but if you get this error the calling code is wrong.) */
    TINFL_STATUS_BAD_PARAM = -3,

    /* This flags indicate the inflator is finished but the adler32 check of the uncompressed data didn't match. If you call it again it'll return TINFL_STATUS_DONE. */
    TINFL_STATUS_ADLER32_MISMATCH = -2,

    /* This flags indicate the inflator has somehow failed (bad code, corrupted input, etc.). If you call it again without resetting via tinfl_init() it it'll just keep on returning the same status failure code. */
    TINFL_STATUS_FAILED = -1,

    /* Any status code less than TINFL_STATUS_DONE must indicate a failure. */

    /* This flag indicates the inflator has returned every byte of uncompressed data that it can, has consumed every byte that it needed, has successfully reached the end of the deflate stream, and */
    /* if zlib headers and adler32 checking enabled that it has successfully checked the uncompressed data's adler32. If you call it again you'll just get TINFL_STATUS_DONE over and over again. */
    TINFL_STATUS_DONE = 0,

    /* This flag indicates the inflator MUST have more input data (even 1 byte) before it can make any more forward progress, or you need to clear the TINFL_FLAG_HAS_MORE_INPUT */
    /* flag on the next call if you don't have any more source data. If the source data was somehow corrupted it's also possible (but unlikely) for the inflator to keep on demanding input to */
    /* proceed, so be sure to properly set the TINFL_FLAG_HAS_MORE_INPUT flag. */
    TINFL_STATUS_NEEDS_MORE_INPUT = 1,

    /* This flag indicates the inflator definitely has 1 or more bytes of uncompressed data available, but it cannot write this data into the output buffer. */
    /* Note if the source compressed data was corrupted it's possible for the inflator to return a lot of uncompressed data to the caller. I've been assuming you know how much uncompressed data to expect */
    /* (either exact or worst case) and will stop calling the inflator and fail after receiving too much. In pure streaming scenarios where you have no idea how many bytes to expect this may not be possible */
    /* so I may need to add some code to address this. */
    TINFL_STATUS_HAS_MORE_OUTPUT = 2,
}

impl From<TINFLStatus> for tinfl_status {
    fn from(status: TINFLStatus) -> tinfl_status {
        use self::tinfl_status::*;
        match status {
            TINFLStatus::FailedCannotMakeProgress => TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS,
            TINFLStatus::BadParam => TINFL_STATUS_BAD_PARAM,
            TINFLStatus::Adler32Mismatch => TINFL_STATUS_ADLER32_MISMATCH,
            TINFLStatus::Failed => TINFL_STATUS_FAILED,
            TINFLStatus::Done => TINFL_STATUS_DONE,
            TINFLStatus::NeedsMoreInput => TINFL_STATUS_NEEDS_MORE_INPUT,
            TINFLStatus::HasMoreOutput => TINFL_STATUS_HAS_MORE_OUTPUT
        }
    }
}

#[allow(bad_style)]
#[repr(C)]
pub struct tinfl_decompressor {
    inner: Option<Box<DecompressorOxide>>,
}

impl Default for tinfl_decompressor {
    fn default() -> tinfl_decompressor {
        tinfl_decompressor {
            inner: Some(Box::<DecompressorOxide>::default()),
        }
    }
}

unmangle!(
    pub unsafe extern "C" fn tinfl_decompress(
        r: *mut tinfl_decompressor,
        in_buf: *const u8,
        in_buf_size: *mut usize,
        out_buf_start: *mut u8,
        out_buf_next: *mut u8,
        out_buf_size: *mut usize,
        flags: u32,
    ) -> i32 {
        let next_pos = out_buf_next as usize - out_buf_start as usize;
        let out_size = *out_buf_size + next_pos;
        let r_ref = r.as_mut().expect("bad decompressor pointer");
        if let Some(decompressor) = r_ref.inner.as_mut() {
            let (status, in_consumed, out_consumed) = decompress(
                decompressor.as_mut(),
                slice::from_raw_parts(in_buf, *in_buf_size),
                slice::from_raw_parts_mut(out_buf_start, out_size),
                next_pos,
                flags,
            );

            *in_buf_size = in_consumed;
            *out_buf_size = out_consumed;
            status as i32
        } else {
            TINFLStatus::BadParam as i32
        }
    }

    pub unsafe extern "C" fn tinfl_decompress_mem_to_mem(
        p_out_buf: *mut c_void,
        out_buf_len: size_t,
        p_src_buf: *const c_void,
        src_buf_len: size_t,
        flags: c_int,
    ) -> size_t {
        let flags = flags as u32;
        let mut decomp = Box::<DecompressorOxide>::default();

        let (status, _, out_consumed) = decompress(
            &mut decomp,
            slice::from_raw_parts(p_src_buf as *const u8, src_buf_len),
            slice::from_raw_parts_mut(p_out_buf as *mut u8, out_buf_len),
            0,
            (flags & !inflate_flags::TINFL_FLAG_HAS_MORE_INPUT)
                | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
        );

        if status != TINFLStatus::Done {
            TINFL_DECOMPRESS_MEM_TO_MEM_FAILED as size_t
        } else {
            out_consumed
        }
    }

    /// Decompress data from `p_src_buf` to a continuously growing heap-allocated buffer.
    ///
    /// Sets `p_out_len` to the length of the returned buffer.
    /// Returns `ptr::null()` if decompression or allocation fails.
    /// The buffer should be freed with `miniz_def_free_func`.
    pub unsafe extern "C" fn tinfl_decompress_mem_to_heap(
        p_src_buf: *const c_void,
        src_buf_len: size_t,
        p_out_len: *mut size_t,
        flags: c_int,
    ) -> *mut c_void {
        let flags = flags as u32;
        const MIN_BUFFER_CAPACITY: size_t = 128;

        // We're not using a Vec for the buffer here to make sure the buffer is allocated and freed by
        // the same allocator.

        let mut decomp = DecompressorOxide::default();
        // Pointer to the buffer to place the decompressed data into.
        let mut p_buf: *mut c_void =
            crate::miniz_def_alloc_func(ptr::null_mut(), MIN_BUFFER_CAPACITY, 1);
        // Capacity of the current output buffer.
        let mut out_buf_capacity = MIN_BUFFER_CAPACITY;

        *p_out_len = 0;
        // How far into the source buffer we have read.
        let mut src_buf_ofs = 0;
        loop {
            let (status, in_consumed, out_consumed) = decompress(
                &mut decomp,
                slice::from_raw_parts(
                    p_src_buf.add(src_buf_ofs) as *const u8,
                    src_buf_len - src_buf_ofs,
                ),
                slice::from_raw_parts_mut(p_buf as *mut u8, out_buf_capacity),
                *p_out_len,
                (flags & !inflate_flags::TINFL_FLAG_HAS_MORE_INPUT)
                    | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
            );

            // If decompression fails or we don't have any input, bail out.
            if (status as i32) < 0 || status == TINFLStatus::NeedsMoreInput {
                crate::miniz_def_free_func(ptr::null_mut(), p_buf);
                *p_out_len = 0;
                return ptr::null_mut();
            }

            src_buf_ofs += in_consumed;
            *p_out_len += out_consumed;

            if status == TINFLStatus::Done {
                break;
            }

            // If we need more space, double the capacity of the output buffer
            // and keep going.
            let mut new_out_buf_capacity = out_buf_capacity * 2;

            // Try to get at least 128 bytes of buffer capacity.
            if new_out_buf_capacity < MIN_BUFFER_CAPACITY {
                new_out_buf_capacity = MIN_BUFFER_CAPACITY
            }

            let p_new_buf =
                crate::miniz_def_realloc_func(ptr::null_mut(), p_buf, 1, new_out_buf_capacity);
            // Bail out if growing fails.
            if p_new_buf.is_null() {
                crate::miniz_def_free_func(ptr::null_mut(), p_buf);
                *p_out_len = 0;
                return ptr::null_mut();
            }

            // Otherwise, continue using the reallocated buffer.
            p_buf = p_new_buf;
            out_buf_capacity = new_out_buf_capacity;
        }

        p_buf
    }

    /// Allocate a compressor.
    ///
    /// This does initialize the struct, but not the inner constructor,
    /// tdefl_init has to be called before doing anything with it.
    pub unsafe extern "C" fn tinfl_decompressor_alloc() -> *mut tinfl_decompressor {
        Box::into_raw(Box::<tinfl_decompressor>::new(tinfl_decompressor::default()))
    }
    /// Deallocate the compressor. (Does nothing if the argument is null).
    ///
    /// This also calles the compressor's destructor, freeing the internal memory
    /// allocated by it.
    pub unsafe extern "C" fn tinfl_decompressor_free(c: *mut tinfl_decompressor) {
        if !c.is_null() {
            Box::from_raw(c);
        }
    }

    pub unsafe extern "C" fn tinfl_init(c: *mut tinfl_decompressor) {
        let wrapped = c.as_mut().unwrap();
        if let Some(decomp) = wrapped.inner.as_mut() {
            decomp.init();
        } else {
            wrapped.inner.replace(Box::default());
        }
    }

    pub unsafe extern "C" fn tinfl_get_adler32(c: *mut tinfl_decompressor) -> c_int {
        let wrapped = c.as_mut().unwrap();
        if let Some(decomp) = wrapped.inner.as_mut() {
            // TODO: Need to test if conversion is ok.
            decomp.adler32().unwrap_or(0) as c_int
        } else {
            0
        }
    }
);

#[cfg(test)]
mod test {
    use miniz_oxide::inflate::core::inflate_flags::{
        TINFL_FLAG_COMPUTE_ADLER32, TINFL_FLAG_PARSE_ZLIB_HEADER,
    };

    use super::*;
    use libc::c_void;
    use std::{ops, slice};
    /// Safe wrapper for `tinfl_decompress_mem_to_mem` using slices.
    ///
    /// Could maybe make this public later.
    fn tinfl_decompress_mem_to_mem_wrapper(
        source: &mut [u8],
        dest: &mut [u8],
        flags: i32,
    ) -> Option<usize> {
        let status = unsafe {
            let source_len = source.len();
            let dest_len = dest.len();
            tinfl_decompress_mem_to_mem(
                dest.as_mut_ptr() as *mut c_void,
                dest_len,
                source.as_mut_ptr() as *const c_void,
                source_len,
                flags,
            )
        };
        if status != TINFL_DECOMPRESS_MEM_TO_MEM_FAILED {
            Some(status)
        } else {
            None
        }
    }

    /// Safe wrapper around a buffer allocated with the miniz_def functions.
    pub struct TinflHeapBuf {
        buf: *mut c_void,
        len: size_t,
    }

    impl TinflHeapBuf {
        fn as_slice(&self) -> &[u8] {
            unsafe { slice::from_raw_parts(self.buf as *const u8, self.len) }
        }
    }

    impl ops::Drop for TinflHeapBuf {
        fn drop(&mut self) {
            unsafe {
                crate::miniz_def_free_func(ptr::null_mut(), self.buf);
            }
        }
    }

    /// Safe wrapper for `tinfl_decompress_mem_to_heap` using slices.
    ///
    /// Could maybe make something like this public later.
    fn tinfl_decompress_mem_to_heap_wrapper(source: &mut [u8], flags: i32) -> Option<TinflHeapBuf> {
        let source_len = source.len();
        let mut out_len = 0;
        unsafe {
            let buf_ptr = tinfl_decompress_mem_to_heap(
                source.as_ptr() as *const c_void,
                source_len,
                &mut out_len,
                flags,
            );
            if !buf_ptr.is_null() {
                Some(TinflHeapBuf {
                    buf: buf_ptr,
                    len: out_len,
                })
            } else {
                None
            }
        }
    }

    #[test]
    fn mem_to_mem() {
        let mut encoded = [
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
        ];
        let mut out_buf = vec![0; 50];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;
        let size = tinfl_decompress_mem_to_mem_wrapper(
            &mut encoded[..],
            out_buf.as_mut_slice(),
            flags as i32,
        )
        .unwrap();
        assert_eq!(&out_buf[..size], &b"Hello, zlib!"[..]);
    }

    #[test]
    fn mem_to_heap() {
        let mut encoded = [
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
        ];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;
        let out_buf = tinfl_decompress_mem_to_heap_wrapper(&mut encoded[..], flags as i32).unwrap();
        assert_eq!(out_buf.as_slice(), &b"Hello, zlib!"[..]);
    }
}
