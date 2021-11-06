use libc::*;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::{cmp, mem, ptr, slice};

use miniz_oxide::deflate::core::{
    compress, compress_to_output, create_comp_flags_from_zip_params, CompressorOxide, TDEFLFlush,
    TDEFLStatus,
};

/// Compression callback function type.
pub type PutBufFuncPtrNotNull = unsafe extern "C" fn(*const c_void, c_int, *mut c_void) -> bool;
/// `Option` alias for compression callback function type.
pub type PutBufFuncPtr = Option<PutBufFuncPtrNotNull>;

pub struct CallbackFunc {
    pub put_buf_func: PutBufFuncPtrNotNull,
    pub put_buf_user: *mut c_void,
}

/// Main compression struct. Not the same as `CompressorOxide`
#[repr(C)]
pub struct Compressor {
    pub(crate) inner: Option<CompressorOxide>,
    pub(crate) callback: Option<CallbackFunc>,
}

impl Default for Compressor {
    fn default() -> Self {
        Compressor {
            inner: None,
            callback: None,
        }
    }
}

#[repr(C)]
#[allow(bad_style)]
#[derive(PartialEq, Eq)]
pub enum tdefl_status {
    TDEFL_STATUS_BAD_PARAM = -2,
    TDEFL_STATUS_PUT_BUF_FAILED = -1,
    TDEFL_STATUS_OKAY = 0,
    TDEFL_STATUS_DONE = 1,
}

impl From<TDEFLStatus> for tdefl_status {
    fn from(status: TDEFLStatus) -> tdefl_status {
        use self::tdefl_status::*;
        match status {
            TDEFLStatus::BadParam => TDEFL_STATUS_BAD_PARAM,
            TDEFLStatus::PutBufFailed => TDEFL_STATUS_PUT_BUF_FAILED,
            TDEFLStatus::Okay => TDEFL_STATUS_OKAY,
            TDEFLStatus::Done => TDEFL_STATUS_DONE,
        }
    }
}

/// Convert an i32 to a TDEFLFlush
///
/// Returns TDEFLFLush::None flush value is unknown.
/// For use with c interface.
pub fn i32_to_tdefl_flush(flush: i32) -> TDEFLFlush {
    match flush {
        2 => TDEFLFlush::Sync,
        3 => TDEFLFlush::Full,
        4 => TDEFLFlush::Finish,
        _ => TDEFLFlush::None,
    }
}

impl Compressor {
    pub(crate) fn new_with_callback(flags: u32, func: CallbackFunc) -> Self {
        Compressor {
            inner: Some(CompressorOxide::new(flags)),
            callback: Some(func),
        }
    }

    /// Sets the inner state to `None` and thus drops it.
    pub fn drop_inner(&mut self) {
        self.inner = None;
    }

    /// Reset the inner compressor if any.
    pub fn reset(&mut self) {
        if let Some(c) = self.inner.as_mut() {
            c.reset();
        }
    }

    pub fn adler32(&self) -> u32 {
        self.inner.as_ref().map(|i| i.adler32()).unwrap_or(0)
    }

    pub fn prev_return_status(&self) -> TDEFLStatus {
        // Not sure we should return on inner not existing, but that shouldn't happen
        // anyway.
        self.inner
            .as_ref()
            .map(|i| i.prev_return_status())
            .unwrap_or(TDEFLStatus::BadParam)
    }

    /// Return the compressor flags of the inner compressor.
    pub fn flags(&self) -> i32 {
        self.inner.as_ref().map(|i| i.flags()).unwrap_or(0)
    }
}

unmangle!(
    pub unsafe extern "C" fn tdefl_compress(
        d: Option<&mut Compressor>,
        in_buf: *const c_void,
        in_size: Option<&mut usize>,
        out_buf: *mut c_void,
        out_size: Option<&mut usize>,
        flush: i32,
    ) -> tdefl_status {
        let flush = i32_to_tdefl_flush(flush);
        match d {
            None => {
                if let Some(size) = in_size {
                    *size = 0
                }
                if let Some(size) = out_size {
                    *size = 0
                }
                tdefl_status::TDEFL_STATUS_BAD_PARAM
            }
            Some(compressor_wrap) => {
                if let Some(ref mut compressor) = compressor_wrap.inner {
                    let in_buf_size = in_size.as_ref().map_or(0, |size| **size);
                    let out_buf_size = out_size.as_ref().map_or(0, |size| **size);

                    if in_buf_size > 0 && in_buf.is_null() {
                        if let Some(size) = in_size {
                            *size = 0
                        }
                        if let Some(size) = out_size {
                            *size = 0
                        }
                        return tdefl_status::TDEFL_STATUS_BAD_PARAM;
                    }

                    let in_slice = (in_buf as *const u8)
                        .as_ref()
                        .map_or(&[][..], |in_buf| slice::from_raw_parts(in_buf, in_buf_size));

                    let res = match compressor_wrap.callback {
                        None => match (out_buf as *mut u8).as_mut() {
                            Some(out_buf) => compress(
                                compressor,
                                in_slice,
                                slice::from_raw_parts_mut(out_buf, out_buf_size),
                                flush,
                            ),
                            None => {
                                if let Some(size) = in_size {
                                    *size = 0
                                }
                                if let Some(size) = out_size {
                                    *size = 0
                                }
                                return tdefl_status::TDEFL_STATUS_BAD_PARAM;
                            }
                        },
                        Some(ref func) => {
                            if out_buf_size > 0 || !out_buf.is_null() {
                                if let Some(size) = in_size {
                                    *size = 0
                                }
                                if let Some(size) = out_size {
                                    *size = 0
                                }
                                return tdefl_status::TDEFL_STATUS_BAD_PARAM;
                            }
                            let res =
                                compress_to_output(compressor, in_slice, flush, |out: &[u8]| {
                                    (func.put_buf_func)(
                                        &(out[0]) as *const u8 as *const c_void,
                                        out.len() as i32,
                                        func.put_buf_user,
                                    )
                                });
                            (res.0, res.1, 0)
                        }
                    };

                    if let Some(size) = in_size {
                        *size = res.1
                    }
                    if let Some(size) = out_size {
                        *size = res.2
                    }
                    res.0.into()
                } else {
                    tdefl_status::TDEFL_STATUS_BAD_PARAM
                }
            }
        }
    }

    pub unsafe extern "C" fn tdefl_compress_buffer(
        d: Option<&mut Compressor>,
        in_buf: *const c_void,
        mut in_size: usize,
        flush: i32,
    ) -> tdefl_status {
        tdefl_compress(d, in_buf, Some(&mut in_size), ptr::null_mut(), None, flush)
    }

    /// Allocate a compressor.
    ///
    /// This does initialize the struct, but not the inner constructor,
    /// tdefl_init has to be called before doing anything with it.
    pub unsafe extern "C" fn tdefl_allocate() -> *mut Compressor {
        Box::into_raw(Box::<Compressor>::new(Compressor {
            inner: None,
            callback: None,
        }))
    }

    /// Deallocate the compressor. (Does nothing if the argument is null).
    ///
    /// This also calles the compressor's destructor, freeing the internal memory
    /// allocated by it.
    pub unsafe extern "C" fn tdefl_deallocate(c: *mut Compressor) {
        if !c.is_null() {
            Box::from_raw(c);
        }
    }

    /// Initialize the compressor struct in the space pointed to by `d`.
    /// if d is null, an error is returned.
    ///
    /// Deinitialization is handled by tdefl_deallocate, and thus
    /// Compressor should not be allocated or freed manually, but only through
    /// tdefl_allocate and tdefl_deallocate
    pub unsafe extern "C" fn tdefl_init(
        d: Option<&mut Compressor>,
        put_buf_func: PutBufFuncPtr,
        put_buf_user: *mut c_void,
        flags: c_int,
    ) -> tdefl_status {
        if let Some(d) = d {
            match catch_unwind(AssertUnwindSafe(|| {
                d.inner = Some(CompressorOxide::new(flags as u32));
                if let Some(f) = put_buf_func {
                    d.callback = Some(CallbackFunc {
                        put_buf_func: f,
                        put_buf_user,
                    })
                } else {
                    d.callback = None;
                };
            })) {
                Ok(_) => tdefl_status::TDEFL_STATUS_OKAY,
                Err(_) => {
                    eprintln!("FATAL ERROR: Caught panic when initializing the compressor!");
                    tdefl_status::TDEFL_STATUS_BAD_PARAM
                }
            }
        } else {
            tdefl_status::TDEFL_STATUS_BAD_PARAM
        }
    }

    pub unsafe extern "C" fn tdefl_get_prev_return_status(
        d: Option<&mut Compressor>,
    ) -> tdefl_status {
        d.map_or(tdefl_status::TDEFL_STATUS_OKAY, |d| {
            d.prev_return_status().into()
        })
    }

    pub unsafe extern "C" fn tdefl_get_adler32(d: Option<&mut Compressor>) -> c_uint {
        d.map_or(crate::MZ_ADLER32_INIT as u32, |d| d.adler32())
    }

    pub unsafe extern "C" fn tdefl_compress_mem_to_output(
        buf: *const c_void,
        buf_len: usize,
        put_buf_func: PutBufFuncPtr,
        put_buf_user: *mut c_void,
        flags: c_int,
    ) -> bool {
        if let Some(put_buf_func) = put_buf_func {
            let compressor =
                crate::miniz_def_alloc_func(ptr::null_mut(), 1, mem::size_of::<Compressor>())
                    as *mut Compressor;

            ptr::write(
                compressor,
                Compressor::new_with_callback(
                    flags as u32,
                    CallbackFunc {
                        put_buf_func,
                        put_buf_user,
                    },
                ),
            );

            let res =
                tdefl_compress_buffer(compressor.as_mut(), buf, buf_len, TDEFLFlush::Finish as i32)
                    == tdefl_status::TDEFL_STATUS_DONE;
            if let Some(c) = compressor.as_mut() {
                c.drop_inner();
            }
            crate::miniz_def_free_func(ptr::null_mut(), compressor as *mut c_void);
            res
        } else {
            false
        }
    }
);

struct BufferUser {
    pub size: usize,
    pub capacity: usize,
    pub buf: *mut u8,
    pub expandable: bool,
}

pub unsafe extern "C" fn output_buffer_putter(
    buf: *const c_void,
    len: c_int,
    user: *mut c_void,
) -> bool {
    let user = (user as *mut BufferUser).as_mut();
    match user {
        None => false,
        Some(user) => {
            let new_size = user.size + len as usize;
            if new_size > user.capacity {
                if !user.expandable {
                    return false;
                }
                let mut new_capacity = cmp::max(user.capacity, 128);
                while new_size > new_capacity {
                    new_capacity <<= 1;
                }

                let new_buf = crate::miniz_def_realloc_func(
                    ptr::null_mut(),
                    user.buf as *mut c_void,
                    1,
                    new_capacity,
                );

                if new_buf.is_null() {
                    return false;
                }

                user.buf = new_buf as *mut u8;
                user.capacity = new_capacity;
            }

            ptr::copy_nonoverlapping(
                buf as *const u8,
                user.buf.add(user.size),
                len as usize,
            );
            user.size = new_size;
            true
        }
    }
}

unmangle!(
    pub unsafe extern "C" fn tdefl_compress_mem_to_heap(
        src_buf: *const c_void,
        src_buf_len: usize,
        out_len: *mut usize,
        flags: c_int,
    ) -> *mut c_void {
        match out_len.as_mut() {
            None => ptr::null_mut(),
            Some(len) => {
                *len = 0;

                let mut buffer_user = BufferUser {
                    size: 0,
                    capacity: 0,
                    buf: ptr::null_mut(),
                    expandable: true,
                };

                if !tdefl_compress_mem_to_output(
                    src_buf,
                    src_buf_len,
                    Some(output_buffer_putter),
                    &mut buffer_user as *mut BufferUser as *mut c_void,
                    flags,
                ) {
                    ptr::null_mut()
                } else {
                    *len = buffer_user.size;
                    buffer_user.buf as *mut c_void
                }
            }
        }
    }

    pub unsafe extern "C" fn tdefl_compress_mem_to_mem(
        out_buf: *mut c_void,
        out_buf_len: usize,
        src_buf: *const c_void,
        src_buf_len: usize,
        flags: c_int,
    ) -> usize {
        if out_buf.is_null() {
            return 0;
        }
        let mut buffer_user = BufferUser {
            size: 0,
            capacity: out_buf_len,
            buf: out_buf as *mut u8,
            expandable: false,
        };

        if tdefl_compress_mem_to_output(
            src_buf,
            src_buf_len,
            Some(output_buffer_putter),
            &mut buffer_user as *mut BufferUser as *mut c_void,
            flags,
        ) {
            buffer_user.size
        } else {
            0
        }
    }

    pub extern "C" fn tdefl_create_comp_flags_from_zip_params(
        level: c_int,
        window_bits: c_int,
        strategy: c_int,
    ) -> c_uint {
        create_comp_flags_from_zip_params(level, window_bits, strategy)
    }
);

#[cfg(test)]
mod test {
    use super::*;
    use miniz_oxide::inflate::decompress_to_vec;

    #[test]
    fn mem_to_heap() {
        let data = b"blargharghawrf31086t13qa9pt7gnseatgawe78vtb6p71v";
        let mut out_len = 0;
        let data_len = data.len();
        let out_data = unsafe {
            let res = tdefl_compress_mem_to_heap(
                data.as_ptr() as *const c_void,
                data_len,
                &mut out_len,
                0,
            );
            assert!(!res.is_null());
            res
        };
        {
            let out_slice = unsafe { slice::from_raw_parts(out_data as *const u8, out_len) };
            let dec = decompress_to_vec(out_slice).unwrap();
            assert!(dec.as_slice() == &data[..]);
        }
    }
}
