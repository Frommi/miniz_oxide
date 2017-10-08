use libc::*;
use std::mem;
use std::cmp;
use std::ptr;

use miniz_oxide::deflate::{compress, create_comp_flags_from_zip_params, CallbackFunc,
                           CallbackOxide, CompressorOxide, PutBufFuncPtr, TDEFLFlush, TDEFLStatus};

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress(
    d: Option<&mut CompressorOxide>,
    in_buf: *const c_void,
    in_size: Option<&mut usize>,
    out_buf: *mut c_void,
    out_size: Option<&mut usize>,
    flush: TDEFLFlush,
) -> TDEFLStatus {
    let res = match d {
        None => {
            in_size.map(|size| *size = 0);
            out_size.map(|size| *size = 0);
            (TDEFLStatus::BadParam, 0, 0)
        }
        Some(compressor) => {
            let callback_res = CallbackOxide::new(
                compressor.callback_func().cloned(),
                in_buf,
                in_size,
                out_buf,
                out_size,
            );

            if let Ok(mut callback) = callback_res {
                let res = compress(compressor, &mut callback, flush);
                callback.update_size(Some(res.1), Some(res.2));
                res
            } else {
                (TDEFLStatus::BadParam, 0, 0)
            }
        }
    };
    res.0
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_buffer(
    d: Option<&mut CompressorOxide>,
    in_buf: *const c_void,
    mut in_size: usize,
    flush: TDEFLFlush,
) -> TDEFLStatus {
    tdefl_compress(d, in_buf, Some(&mut in_size), ptr::null_mut(), None, flush)
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_init(
    d: Option<&mut CompressorOxide>,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> TDEFLStatus {
    if let Some(d) = d {
        *d = CompressorOxide::new(
            put_buf_func.map(|func| {
                CallbackFunc {
                    put_buf_func: func,
                    put_buf_user: put_buf_user,
                }
            }),
            flags as u32,
        );
        TDEFLStatus::Okay
    } else {
        TDEFLStatus::BadParam
    }
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_get_prev_return_status(
    d: Option<&mut CompressorOxide>,
) -> TDEFLStatus {
    d.map_or(TDEFLStatus::Okay, |d| d.prev_return_status())
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_get_adler32(d: Option<&mut CompressorOxide>) -> c_uint {
    d.map_or(::MZ_ADLER32_INIT as u32, |d| d.adler32())
}

#[no_mangle]
pub unsafe extern "C" fn tdefl_compress_mem_to_output(
    buf: *const c_void,
    buf_len: usize,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> bool {
    if let Some(put_buf_func) = put_buf_func {
        let compressor =
            ::miniz_def_alloc_func(ptr::null_mut(), 1, mem::size_of::<CompressorOxide>()) as
                *mut CompressorOxide;

        *compressor = CompressorOxide::new(
            Some(CallbackFunc {
                put_buf_func: put_buf_func,
                put_buf_user: put_buf_user,
            }),
            flags as u32,
        );

        let res = tdefl_compress_buffer(compressor.as_mut(), buf, buf_len, TDEFLFlush::Finish) ==
            TDEFLStatus::Done;
        ::miniz_def_free_func(ptr::null_mut(), compressor as *mut c_void);
        res
    } else {
        false
    }
}

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

                let new_buf = ::miniz_def_realloc_func(
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
                user.buf.offset(user.size as isize),
                len as usize,
            );
            user.size = new_size;
            true
        }
    }
}

#[no_mangle]
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
            )
            {
                ptr::null_mut()
            } else {
                *len = buffer_user.size;
                buffer_user.buf as *mut c_void
            }
        }
    }
}

#[no_mangle]
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
    )
    {
        buffer_user.size
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn tdefl_create_comp_flags_from_zip_params(
    level: c_int,
    window_bits: c_int,
    strategy: c_int,
) -> c_uint {
    create_comp_flags_from_zip_params(level, window_bits, strategy)
}
