use super::*;

use tdef::TDEFL_COMPUTE_ADLER32;
use tdef::tdefl_get_adler32_oxide;
use tdef::TDEFLStatus;

use tinfl::TINFL_STATUS_DONE;
use tinfl::TINFL_STATUS_FAILED;


pub fn mz_adler32_oxide(adler: c_uint, data: &[u8]) -> c_uint {
    let mut s1 = adler & 0xffff;
    let mut s2 = adler >> 16;
    for &x in data {
        s1 = (s1 + x as c_uint) % 65521;
        s2 = (s1 + s2) % 65521;
    }
    (s2 << 16) + s1
}

static S_CRC32: [c_uint; 16] = [0, 0x1db71064, 0x3b6e20c8, 0x26d930ac, 0x76dc4190,
    0x6b6b51f4, 0x4db26158, 0x5005713c, 0xedb88320, 0xf00f9344, 0xd6d6a3e8,
    0xcb61b38c, 0x9b64c2b0, 0x86d3d2d4, 0xa00ae278, 0xbdbdf21c];

pub fn mz_crc32_oxide(crc32: c_uint, data: &[u8]) -> c_uint {
    !data.iter().fold(!crc32, |mut crcu32, &b| {
        crcu32 = (crcu32 >> 4) ^ S_CRC32[(((crcu32 & 0xF) as u8) ^ (b & 0xF)) as usize];
        (crcu32 >> 4) ^ S_CRC32[(((crcu32 & 0xF) as u8) ^ (b >> 4)) as usize]
    })
}

pub trait StateType {}
impl StateType for tdefl_compressor {}
impl StateType for inflate_state {}

pub struct Allocator {
    pub alloc: mz_alloc_func,
    pub free: mz_free_func,
    pub opaque: *mut c_void,
}

pub struct StreamOxide<'io, 'state, ST: 'state> {
    pub next_in: Option<&'io [u8]>,
    pub total_in: c_ulong,

    pub next_out: Option<&'io mut [u8]>,
    pub total_out: c_ulong,

    pub state: Option<&'state mut ST>,

    pub allocator: Allocator,

    pub adler: c_ulong
}

impl Allocator {
    pub fn new(stream: &mut mz_stream) -> Self {
        Allocator {
            alloc: stream.zalloc.unwrap_or(miniz_def_alloc_func),
            free: stream.zfree.unwrap_or(miniz_def_free_func),
            opaque: stream.opaque
        }
    }

    fn alloc_one<'a, T>(&mut self) -> Option<&'a mut T> {
        unsafe { ((self.alloc)(self.opaque, 1, mem::size_of::<T>()) as *mut T).as_mut() }
    }

    fn free<T>(&mut self, ptr: *mut T) {
        unsafe { (self.free)(self.opaque, ptr as *mut c_void) }
    }
}

impl<'io, 'state, ST: StateType> StreamOxide<'io, 'state, ST> {
    pub unsafe fn new(stream: &mut mz_stream) -> Self {
        let in_slice = stream.next_in.as_ref().map(|ptr| {
            slice::from_raw_parts(ptr, stream.avail_in as usize)
        });

        let out_slice = stream.next_out.as_mut().map(|ptr| {
            slice::from_raw_parts_mut(ptr, stream.avail_out as usize)
        });

        StreamOxide {
            next_in: in_slice,
            total_in: stream.total_in,
            next_out: out_slice,
            total_out: stream.total_out,
            state: (stream.state as *mut ST).as_mut(),
            allocator: Allocator::new(stream),
            adler: stream.adler
        }
    }

    pub fn as_mz_stream(&mut self) -> mz_stream {
        mz_stream {
            next_in: self.next_in.map_or(ptr::null(), |in_slice| in_slice.as_ptr()),
            avail_in: self.next_in.map_or(0, |in_slice| in_slice.len() as c_uint),
            total_in: self.total_in,

            next_out: self.next_out.as_mut().map_or(ptr::null_mut(), |out_slice| out_slice.as_mut_ptr()),
            avail_out: self.next_out.as_mut().map_or(0, |out_slice| out_slice.len() as c_uint),
            total_out: self.total_out,

            msg: ptr::null(),

            state: self.state.as_mut().map_or(ptr::null_mut(), |state| {
                (*state as *mut ST) as *mut mz_internal_state
            }),

            zalloc: Some(self.allocator.alloc),
            zfree: Some(self.allocator.free),
            opaque: self.allocator.opaque,

            data_type: 0,
            adler: self.adler,
            reserved: 0
        }
    }
}

fn invalid_window_bits(window_bits: c_int) -> bool {
    (window_bits != MZ_DEFAULT_WINDOW_BITS) && (-window_bits != MZ_DEFAULT_WINDOW_BITS)
}

pub fn mz_compress2_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>,
                          level: c_int,
                          dest_len: &mut c_ulong) -> MZResult
{
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


pub fn mz_deflate_init_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>, level: c_int) -> MZResult {
    mz_deflate_init2_oxide(stream_oxide, level, MZ_DEFLATED, MZ_DEFAULT_WINDOW_BITS, 9, CompressionStrategy::Default as c_int)
}

pub fn mz_deflate_init2_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>,
                              level: c_int,
                              method: c_int,
                              window_bits: c_int,
                              mem_level: c_int,
                              strategy: c_int) -> MZResult
{
    let comp_flags = TDEFL_COMPUTE_ADLER32 as c_uint |
            tdef::tdefl_create_comp_flags_from_zip_params_oxide(level, window_bits, strategy);

    let invalid_level = (mem_level < 1) || (mem_level > 9);
    if (method != MZ_DEFLATED) || invalid_level || invalid_window_bits(window_bits) {
        return Err(MZError::Param);
    }

    stream_oxide.adler = MZ_ADLER32_INIT;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    let mut state = stream_oxide.allocator.alloc_one().ok_or(MZError::Mem)?;
    let status = unsafe { tdef::tdefl_init(state, None, ptr::null_mut(), comp_flags as c_int) };
    if status != TDEFLStatus::Okay as c_int {
        mz_deflate_end_oxide(stream_oxide)?;
        return Err(MZError::Param);
    }
    stream_oxide.state = Some(state);

    Ok(MZStatus::Ok)
}

pub fn mz_deflate_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>, flush: c_int) -> MZResult {
    let mut state = stream_oxide.state.as_mut().ok_or(MZError::Stream)?;
    let mut next_in = stream_oxide.next_in.as_mut().ok_or(MZError::Stream)?;
    let mut next_out = stream_oxide.next_out.as_mut().ok_or(MZError::Stream)?;

    let flush = MZFlush::new(flush)?;

    if next_out.is_empty() {
        return Err(MZError::Buf);
    }

    if state.m_prev_return_status == TDEFLStatus::Done {
        return if flush == MZFlush::Finish {
            Ok(MZStatus::StreamEnd)
        } else {
            Err(MZError::Buf)
        };
    }

    let original_total_in = stream_oxide.total_in;
    let original_total_out = stream_oxide.total_out;

    loop {
        let mut in_bytes = next_in.len();
        let mut out_bytes = next_out.len();
        let defl_status = unsafe { tdef::tdefl_compress(
            *state,
            next_in.as_ptr() as *const c_void,
            &mut in_bytes,
            next_out.as_mut_ptr() as *mut c_void,
            &mut out_bytes,
            flush as c_int
        ) };

        *next_in = &next_in[in_bytes..];
        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
        stream_oxide.total_in += in_bytes as c_ulong;
        stream_oxide.total_out += out_bytes as c_ulong;
        stream_oxide.adler = tdefl_get_adler32_oxide(*state) as c_ulong;

        if defl_status < 0 {
            return Err(MZError::Stream);
        }

        if defl_status == TDEFLStatus::Done as c_int {
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
    if let Some(state) = stream_oxide.state.as_mut() {
        stream_oxide.allocator.free(*state);
    }
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

pub fn mz_inflate_init_oxide(stream_oxide: &mut StreamOxide<inflate_state>) -> MZResult {
    mz_inflate_init2_oxide(stream_oxide, MZ_DEFAULT_WINDOW_BITS)
}

pub fn mz_inflate_init2_oxide(stream_oxide: &mut StreamOxide<inflate_state>,
                              window_bits: c_int) -> MZResult
{
    if invalid_window_bits(window_bits) {
        return Err(MZError::Param);
    }

    stream_oxide.adler = 0;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    let mut state = stream_oxide.allocator.alloc_one::<inflate_state>().ok_or(MZError::Mem)?;
    state.m_decomp.m_state = 0;
    state.m_dict_ofs = 0;
    state.m_dict_avail = 0;
    state.m_last_status = tinfl::TINFL_STATUS_NEEDS_MORE_INPUT;
    state.m_first_call = 1;
    state.m_has_flushed = 0;
    state.m_window_bits = window_bits;
    stream_oxide.state = Some(state);

    Ok(MZStatus::Ok)
}

fn push_dict_out(state: &mut inflate_state, next_out: &mut &mut [u8]) -> c_ulong {
    let n = cmp::min(state.m_dict_avail as usize, next_out.len());
    (next_out[..n]).copy_from_slice(&state.m_dict[state.m_dict_ofs as usize..state.m_dict_ofs as usize + n]);
    *next_out = &mut mem::replace(next_out, &mut [])[n..];
    state.m_dict_avail -= n as c_uint;
    state.m_dict_ofs = (state.m_dict_ofs + (n as c_uint)) & ((tinfl::TINFL_LZ_DICT_SIZE - 1) as c_uint);
    n as c_ulong
}

pub fn mz_inflate_oxide(stream_oxide: &mut StreamOxide<inflate_state>, flush: c_int) -> MZResult {
    let mut state = stream_oxide.state.as_mut().ok_or(MZError::Stream)?;
    let mut next_in = stream_oxide.next_in.as_mut().ok_or(MZError::Stream)?;
    let mut next_out = stream_oxide.next_out.as_mut().ok_or(MZError::Stream)?;

    let flush = MZFlush::new(flush)?;
    if flush == MZFlush::Full {
        return Err(MZError::Stream);
    }

    let mut decomp_flags = tinfl::TINFL_FLAG_COMPUTE_ADLER32;
    if state.m_window_bits > 0 {
        decomp_flags |= tinfl::TINFL_FLAG_PARSE_ZLIB_HEADER;
    }

    let first_call = state.m_first_call;
    state.m_first_call = 0;
    if state.m_last_status < 0 {
        return Err(MZError::Data);
    }

    if (state.m_has_flushed != 0) && (flush != MZFlush::Finish) {
        return Err(MZError::Stream);
    }
    state.m_has_flushed |= (flush == MZFlush::Finish) as c_uint;

    let orig_avail_in = next_in.len() as size_t;

    if (flush == MZFlush::Finish) && (first_call != 0) {
        decomp_flags |= tinfl::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
        let mut in_bytes = next_in.len() as size_t;
        let mut out_bytes = next_out.len() as size_t;

        let status = unsafe { tinfl::tinfl_decompress(
            &mut state.m_decomp,
            next_in.as_ptr(),
            &mut in_bytes,
            next_out.as_mut_ptr(),
            next_out.as_mut_ptr(),
            &mut out_bytes,
            decomp_flags as c_uint
        ) };

        state.m_last_status = status;

        *next_in = &next_in[in_bytes..];
        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
        stream_oxide.total_in += in_bytes as c_ulong;
        stream_oxide.total_out += out_bytes as c_ulong;
        stream_oxide.adler = state.m_decomp.m_check_adler32 as c_ulong;

        if status < 0 {
            return Err(MZError::Data);
        } else if status != TINFL_STATUS_DONE {
            state.m_last_status = TINFL_STATUS_FAILED;
            return Err(MZError::Buf);
        }
        return Ok(MZStatus::StreamEnd);
    }

    if flush != MZFlush::Finish {
        decomp_flags |= tinfl::TINFL_FLAG_HAS_MORE_INPUT;
    }

    if state.m_dict_avail != 0 {
        stream_oxide.total_out += push_dict_out(state, next_out);
        return if (state.m_last_status == TINFL_STATUS_DONE) && (state.m_dict_avail == 0) {
            Ok(MZStatus::StreamEnd)
        } else {
            Ok(MZStatus::Ok)
        };
    }

    loop {
        let mut in_bytes = next_in.len() as usize;
        let mut out_bytes = tinfl::TINFL_LZ_DICT_SIZE - state.m_dict_ofs as usize;

        let status = unsafe { tinfl::tinfl_decompress(
            &mut state.m_decomp,
            next_in.as_ptr(),
            &mut in_bytes,
            state.m_dict.as_mut_ptr(),
            state.m_dict.as_mut_ptr().offset(state.m_dict_ofs as isize),
            &mut out_bytes,
            decomp_flags as c_uint
        ) };

        state.m_last_status = status;

        *next_in = &next_in[in_bytes..];
        stream_oxide.total_in += in_bytes as c_ulong;

        state.m_dict_avail = out_bytes as c_uint;
        stream_oxide.total_out += push_dict_out(state, next_out);
        stream_oxide.adler = state.m_decomp.m_check_adler32 as c_ulong;

        if status < 0 {
            return Err(MZError::Data);
        }

        if (status == tinfl::TINFL_STATUS_NEEDS_MORE_INPUT) && (orig_avail_in == 0) {
            return Err(MZError::Buf);
        }

        if flush == MZFlush::Finish {
            if status == TINFL_STATUS_DONE {
                return if state.m_dict_avail != 0 { Err(MZError::Buf) } else { Ok(MZStatus::StreamEnd) };
            } else if next_out.is_empty() {
                return Err(MZError::Buf);
            }
        } else {
            let empty_buf = next_in.is_empty() || next_out.is_empty();
            if (status == TINFL_STATUS_DONE) || empty_buf || (state.m_dict_avail != 0) {
                return if (status == TINFL_STATUS_DONE) && (state.m_dict_avail == 0) {
                    Ok(MZStatus::StreamEnd)
                } else {
                    Ok(MZStatus::Ok)
                }
            }
        }
    }
}

pub fn mz_inflate_end_oxide(stream_oxide: &mut StreamOxide<inflate_state>) -> MZResult {
    if let Some(state) = stream_oxide.state.as_mut() {
        stream_oxide.allocator.free(*state);
    }
    Ok(MZStatus::Ok)
}


// TODO: probably not covered by tests
pub fn mz_deflate_reset_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>) -> MZResult {
    let mut compressor_state = stream_oxide.state.as_mut().ok_or(MZError::Stream)?;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;
    unsafe {
        tdef::tdefl_init(*compressor_state, None, ptr::null_mut(), compressor_state.m_flags as c_int);
    }

    Ok(MZStatus::Ok)
}
