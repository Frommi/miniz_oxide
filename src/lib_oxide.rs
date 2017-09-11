use super::*;

use crc::{crc32, Hasher32};

use miniz_oxide::inflate;
pub use miniz_oxide::lib_oxide::{mz_inflate_init_oxide,
                               mz_inflate_init2_oxide,
                               mz_inflate_end_oxide,
                                 mz_inflate_oxide,
};

pub use miniz_oxide::lib_oxide::update_adler32 as mz_adler32_oxide;

use tdef::TDEFL_COMPUTE_ADLER32;
use tdef::TDEFLStatus;
use tdef::TDEFLFlush;


pub trait StateType {}
impl StateType for tdefl_compressor {}
impl StateType for inflate_state {}

pub struct BoxedState<ST: StateType> {
    pub inner: *mut ST,
    pub alloc: mz_alloc_func,
    pub free: mz_free_func,
    pub opaque: *mut c_void,
}

impl<ST: StateType> Drop for BoxedState<ST> {
    fn drop(&mut self) {
        self.free_state();
    }
}

impl<ST: StateType> BoxedState<ST> {
    pub fn as_ref(&self) -> Option<&ST> {
        unsafe {
            self.inner.as_ref()
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut ST> {
        unsafe {
            self.inner.as_mut()
        }
    }

    pub fn new(stream: &mut mz_stream) -> Self {
        BoxedState {
            inner: stream.state as *mut ST,
            alloc: stream.zalloc.unwrap_or(miniz_def_alloc_func),
            free: stream.zfree.unwrap_or(miniz_def_free_func),
            opaque: stream.opaque
        }
    }

    pub fn forget(mut self) -> *mut ST {
        let state = self.inner;
        self.inner = ptr::null_mut();
        state
    }

    fn alloc_state<'a, T>(&mut self) -> MZResult {
        self.inner = unsafe { (self.alloc)(self.opaque, 1, mem::size_of::<ST>()) as *mut ST };
        if self.inner.is_null() {
            Err(MZError::Mem)
        } else {
            Ok(MZStatus::Ok)
        }
    }

    pub fn free_state(&mut self) {
        if !self.inner.is_null() {
            unsafe { (self.free)(self.opaque, self.inner as *mut c_void) }
            self.inner = ptr::null_mut();
        }
    }
}


fn invalid_window_bits(window_bits: c_int) -> bool {
    (window_bits != MZ_DEFAULT_WINDOW_BITS) && (-window_bits != MZ_DEFAULT_WINDOW_BITS)
}

pub fn mz_compress2_oxide(
    stream_oxide: &mut StreamOxide<tdefl_compressor>,
    level: c_int,
    dest_len: &mut c_ulong
) -> MZResult {
    mz_deflate_init_oxide(stream_oxide, level)?;
    let status = mz_deflate_oxide(stream_oxide, MZFlush::Finish as c_int);
    mz_deflate_end_oxide(stream_oxide)?;

    match status {
        Ok(MZStatus::StreamEnd) => {
            *dest_len = stream_oxide.total_out;
            Ok(MZStatus::Ok)
        },
        Ok(MZStatus::Ok) => Err(MZError::Buf),
        _ => status
    }
}


pub fn mz_deflate_init_oxide(
    stream_oxide: &mut StreamOxide<tdefl_compressor>,
    level: c_int
) -> MZResult {
    mz_deflate_init2_oxide(
        stream_oxide,
        level,
        MZ_DEFLATED,
        MZ_DEFAULT_WINDOW_BITS,
        9,
        CompressionStrategy::Default as c_int
    )
}

pub fn mz_deflate_init2_oxide(
    stream_oxide: &mut StreamOxide<tdefl_compressor>,
    level: c_int,
    method: c_int,
    window_bits: c_int,
    mem_level: c_int,
    strategy: c_int
) -> MZResult {
    let comp_flags = TDEFL_COMPUTE_ADLER32 as c_uint |
            tdef::create_comp_flags_from_zip_params(level, window_bits, strategy);

    let invalid_level = (mem_level < 1) || (mem_level > 9);
    if (method != MZ_DEFLATED) || invalid_level || invalid_window_bits(window_bits) {
        return Err(MZError::Param);
    }

    stream_oxide.adler = MZ_ADLER32_INIT;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    stream_oxide.state.alloc_state::<tdefl_compressor>()?;
    if stream_oxide.state.as_mut().is_none() {
        mz_deflate_end_oxide(stream_oxide)?;
        return Err(MZError::Param);
    }

    match stream_oxide.state.as_mut() {
        Some(state) => *state = tdefl_compressor::new(None, comp_flags),
        None => unreachable!(),
    }

    Ok(MZStatus::Ok)
}

pub fn mz_deflate_oxide(
    stream_oxide: &mut StreamOxide<tdefl_compressor>,
    flush: c_int
) -> MZResult {
    let state = stream_oxide.state.as_mut().ok_or(MZError::Stream)?;
    let next_in = stream_oxide.next_in.as_mut().ok_or(MZError::Stream)?;
    let next_out = stream_oxide.next_out.as_mut().ok_or(MZError::Stream)?;

    let flush = MZFlush::new(flush)?;

    if next_out.is_empty() {
        return Err(MZError::Buf);
    }

    if state.get_prev_return_status() == TDEFLStatus::Done {
        return if flush == MZFlush::Finish {
            Ok(MZStatus::StreamEnd)
        } else {
            Err(MZError::Buf)
        };
    }

    let original_total_in = stream_oxide.total_in;
    let original_total_out = stream_oxide.total_out;

    loop {
        let in_bytes;
        let out_bytes;
        let defl_status = {
            let mut callback = tdef::CallbackOxide::new_callback_buf(*next_in, *next_out);
            let res = tdef::compress(state, &mut callback, TDEFLFlush::from(flush));
            in_bytes = res.1;
            out_bytes = res.2;
            res.0
        };

        *next_in = &next_in[in_bytes..];
        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
        stream_oxide.total_in += in_bytes as c_ulong;
        stream_oxide.total_out += out_bytes as c_ulong;
        stream_oxide.adler = state.get_adler32() as c_ulong;

        if defl_status == TDEFLStatus::BadParam || defl_status == TDEFLStatus::PutBufFailed {
            return Err(MZError::Stream);
        }

        if defl_status == TDEFLStatus::Done {
            return Ok(MZStatus::StreamEnd);
        }

        if next_out.is_empty() {
            return Ok(MZStatus::Ok);
        }

        if next_in.is_empty() && (flush != MZFlush::Finish) {
            let total_changed = (stream_oxide.total_in != original_total_in) ||
                (stream_oxide.total_out != original_total_out);

            return if (flush != MZFlush::None) || total_changed {
                 Ok(MZStatus::Ok)
            } else {
                Err(MZError::Buf)
            }
        }
    }
}

pub fn mz_deflate_end_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>) -> MZResult {
    stream_oxide.state.free_state();
    Ok(MZStatus::Ok)
}

pub fn mz_uncompress2_oxide(stream_oxide: &mut StreamOxide<inflate_state>,
                            dest_len: &mut c_ulong) -> MZResult
{
    mz_inflate_init_oxide(stream_oxide)?;
    let status = mz_inflate_oxide(stream_oxide, MZFlush::Finish as c_int);
    mz_inflate_end_oxide(stream_oxide)?;

    let empty_in = stream_oxide.next_in.map_or(true, |next_in| next_in.is_empty());
    match (status, empty_in) {
        (Ok(MZStatus::StreamEnd), _) => {
            *dest_len = stream_oxide.total_out;
            Ok(MZStatus::Ok)
        },
        (Err(MZError::Buf), true) => Err(MZError::Data),
        (status, _) => status
    }
}

fn push_dict_out(state: &mut inflate_state, next_out: &mut &mut [u8]) -> c_ulong {
    let n = cmp::min(state.m_dict_avail as usize, next_out.len());
    (next_out[..n]).copy_from_slice(&state.m_dict[state.m_dict_ofs as usize..state.m_dict_ofs as usize + n]);
    *next_out = &mut mem::replace(next_out, &mut [])[n..];
    state.m_dict_avail -= n as c_uint;
    state.m_dict_ofs = (state.m_dict_ofs + (n as c_uint)) & ((inflate::TINFL_LZ_DICT_SIZE - 1) as c_uint);
    n as c_ulong
}

// TODO: probably not covered by tests
pub fn mz_deflate_reset_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>) -> MZResult {
    let state = stream_oxide.state.as_mut().ok_or(MZError::Stream)?;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;
    *state = tdef::CompressorOxide::new(None, state.get_flags() as u32);
    Ok(MZStatus::Ok)
}
