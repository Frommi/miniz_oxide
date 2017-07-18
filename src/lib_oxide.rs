use super::*;

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


#[repr(C)]
#[allow(bad_style)]
pub struct inflate_state {
    pub m_decomp: tinfl_decompressor,

    pub m_dict_ofs: c_uint,
    pub m_dict_avail: c_uint,
    pub m_first_call: c_uint,
    pub m_has_flushed: c_uint,

    pub m_window_bits: c_int,
    pub m_dict: [u8; tinfl::TINFL_LZ_DICT_SIZE],
    pub m_last_status: c_int
}

pub trait StateType {}
impl StateType for tdefl_compressor {}
impl StateType for inflate_state {}

pub struct StreamOxide<'io, 'state, ST: 'state> {
    pub next_in: Option<&'io [u8]>,
    pub total_in: c_ulong,

    pub next_out: Option<&'io mut [u8]>,
    pub total_out: c_ulong,

    pub state: Option<&'state mut ST>,

    pub alloc: Option<mz_alloc_func>,
    pub free: Option<mz_free_func>,
    pub opaque: *mut c_void,

    pub adler: c_ulong
}

impl<'io, 'state, ST: StateType> StreamOxide<'io, 'state, ST> {
    pub unsafe fn new(stream: &mut mz_stream) -> Self {
        let in_slice = match stream.next_in.as_ref() {
            None => None,
            Some(ptr) => Some(slice::from_raw_parts(ptr, stream.avail_in as usize))
        };
        let out_slice = match stream.next_out.as_mut() {
            None => None,
            Some(ptr) => Some(slice::from_raw_parts_mut(ptr, stream.avail_out as usize))
        };

        StreamOxide {
            next_in: in_slice,
            total_in: stream.total_in,
            next_out: out_slice,
            total_out: stream.total_out,
            state: (stream.state as *mut ST).as_mut(),
            alloc: stream.zalloc,
            free: stream.zfree,
            opaque: stream.opaque,
            adler: stream.adler
        }
    }

    pub fn as_mz_stream(&mut self) -> mz_stream {
        mz_stream {
            next_in: self.next_in.map_or(ptr::null(), |in_slice| in_slice.as_ptr()),
            avail_in: self.next_in.map_or(0, |in_slice| in_slice.len() as c_uint),
            total_in: self.total_in,
            next_out: match self.next_out {
                None => ptr::null_mut(),
                Some(ref mut out_slice) => out_slice.as_mut_ptr()
            },
            avail_out: match self.next_out {
                None => 0,
                Some(ref mut out_slice) => out_slice.len() as c_uint
            },
            total_out: self.total_out,
            msg: ptr::null(),
            state: match self.state {
                None => ptr::null_mut(),
                Some(ref mut state) => (*state as *mut ST) as *mut mz_internal_state
            },
            zalloc: self.alloc,
            zfree: self.free,
            opaque: self.opaque,
            data_type: 0,
            adler: self.adler,
            reserved: 0
        }
    }

    pub fn set_alloc_if_none(&mut self, f: mz_alloc_func) {
        if self.alloc.is_none() {
            self.alloc = Some(f);
        }
    }

    pub fn set_free_if_none(&mut self, f: mz_free_func) {
        if self.free.is_none() {
            self.free = Some(f);
        }
    }

    pub unsafe fn use_free(&self, address: *mut c_void) {
        self.free.map(|free| free(self.opaque, address));
    }
}


macro_rules! alloc_one {
    ($stream_oxide:expr, $T:ty) => (
        match $stream_oxide.alloc {
            None => None,
            Some(alloc) => (alloc($stream_oxide.opaque, 1, mem::size_of::<$T>()) as *mut $T).as_mut()
        }
    )
}

macro_rules! alloc_array {
    ($stream_oxide:expr, $T:ty, $items:expr) => (
        match $stream_oxide.alloc {
            None => None,
            Some(alloc) => Some(
                slice::from_raw_parts_mut(
                    alloc($stream_oxide.opaque, $items, mem::size_of::<$T>()) as *mut $T,
                    $items
                )
            )
        }
    )
}

macro_rules! free {
    ($stream_oxide:expr, $ptr:expr) => (
        match $stream_oxide.free {
            None => (),
            Some(free) => { free($stream_oxide.opaque, $ptr); }
        }
    )
}

pub fn mz_compress2_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>,
                          level: c_int,
                          dest_len: &mut c_ulong) -> Result<MZStatus, MZError>
{
    mz_deflate_init_oxide(stream_oxide, level)?;
    let status = mz_deflate_oxide(stream_oxide, MZ_FINISH);
    mz_deflate_end_oxide(stream_oxide)?;

    match status {
        Ok(MZStatus::StreamEnd) => {
            *dest_len = stream_oxide.total_out;
            Ok(MZStatus::Ok)
        },
        Ok(MZStatus::Ok) => { Err(MZError::Buf) },
        _ => status
    }
}


use tdef::TDEFL_COMPUTE_ADLER32;
use tdef::TDEFL_STATUS_OKAY;
use tdef::TDEFL_STATUS_DONE;
use tdef::tdefl_get_adler32_oxide;

pub fn mz_deflate_init_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>, level: c_int) -> Result<MZStatus, MZError> {
    mz_deflate_init2_oxide(stream_oxide, level, MZ_DEFLATED, MZ_DEFAULT_WINDOW_BITS, 9, CompressionStrategy::Default as c_int)
}

pub fn mz_deflate_init2_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>,
                              level: c_int,
                              method: c_int,
                              window_bits: c_int,
                              mem_level: c_int,
                              strategy: c_int) -> Result<MZStatus, MZError>
{
    let comp_flags = TDEFL_COMPUTE_ADLER32 as c_uint |
            tdef::tdefl_create_comp_flags_from_zip_params_oxide(level, window_bits, strategy);

    if (method != MZ_DEFLATED) || ((mem_level < 1) || (mem_level > 9)) ||
        ((window_bits != MZ_DEFAULT_WINDOW_BITS) && (-window_bits != MZ_DEFAULT_WINDOW_BITS)) {
        return Err(MZError::Param);
    }

    stream_oxide.adler = MZ_ADLER32_INIT;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    stream_oxide.set_alloc_if_none(miniz_def_alloc_func);
    stream_oxide.set_free_if_none(miniz_def_free_func);

    unsafe {
        alloc_one!(stream_oxide, tdefl_compressor)
    }.map_or(Err(MZError::Mem), |compressor_state| {
        if unsafe {
            tdef::tdefl_init(compressor_state, None, ptr::null_mut(), comp_flags as c_int)
        } != TDEFL_STATUS_OKAY {
            mz_deflate_end_oxide(stream_oxide)?;
            return Err(MZError::Param);
        }
        stream_oxide.state = Some(compressor_state);
        Ok(MZStatus::Ok)
    })
}

pub fn mz_deflate_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>, flush: c_int) -> Result<MZStatus, MZError> {
    match stream_oxide.state {
        None => Err(MZError::Stream),
        Some(ref mut state) => {
            match (flush, &mut stream_oxide.next_in, &mut stream_oxide.next_out) {
                (mut flush @ 0 ... MZ_FINISH, &mut Some(ref mut next_in), &mut Some(ref mut next_out)) => {
                    if next_out.len() == 0 {
                        return Err(MZError::Buf);
                    }

                    if flush == MZ_PARTIAL_FLUSH {
                        flush = MZ_SYNC_FLUSH;
                    }

                    if state.m_prev_return_status == TDEFL_STATUS_DONE {
                        return Err(if flush == MZ_FINISH { MZError::Stream } else { MZError::Buf });
                    }

                    let original_total_in = stream_oxide.total_in;
                    let original_total_out = stream_oxide.total_out;

                    let mut status = Ok(MZStatus::Ok);
                    loop {
                        let mut in_bytes = next_in.len();
                        let mut out_bytes = next_out.len();
                        let defl_status = unsafe {
                            tdef::tdefl_compress(
                                *state,
                                next_in.as_ptr() as *const c_void,
                                &mut in_bytes,
                                next_out.as_mut_ptr() as *mut c_void,
                                &mut out_bytes,
                                flush
                            )
                        };

                        *next_in = &next_in[in_bytes..];
                        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
                        stream_oxide.total_in += in_bytes as c_ulong;
                        stream_oxide.total_out += out_bytes as c_ulong;
                        stream_oxide.adler = tdefl_get_adler32_oxide(*state) as c_ulong;

                        if defl_status < 0 {
                            status = Err(MZError::Stream);
                            break;
                        } else if defl_status == TDEFL_STATUS_DONE {
                            status = Ok(MZStatus::StreamEnd);
                            break;
                        } else if next_out.len() == 0 {
                            break;
                        } else if (next_in.len() == 0) && (flush != MZ_FINISH) {
                            if (flush != 0) || (stream_oxide.total_in != original_total_in) ||
                                (stream_oxide.total_out != original_total_out) {
                                break;
                            }
                            return Err(MZError::Buf);
                        }
                    }
                    status
                },
                _ => Err(MZError::Stream)
            }
        }
    }
}

pub fn mz_deflate_end_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>) -> Result<MZStatus, MZError> {
    match stream_oxide.state {
        None => (),
        Some(ref mut state) => unsafe {
            free!(stream_oxide, (*state as *mut tdefl_compressor) as *mut c_void)
        },
    };
    Ok(MZStatus::Ok)
}

pub fn mz_uncompress2_oxide(stream_oxide: &mut StreamOxide<inflate_state>,
                            dest_len: &mut c_ulong) -> Result<MZStatus, MZError>
{
    mz_inflate_init_oxide(stream_oxide)?;
    let status = mz_inflate_oxide(stream_oxide, MZ_FINISH);
    mz_inflate_end_oxide(stream_oxide)?;

    match (status, stream_oxide.next_in.map_or(0, |next_in| next_in.len())) {
        (Ok(MZStatus::StreamEnd), _) => {
            *dest_len = stream_oxide.total_out;
            Ok(MZStatus::Ok)
        },
        (Err(MZError::Buf), 0) => Err(MZError::Data),
        (status, _) => status
    }
}

pub fn mz_inflate_init_oxide(stream_oxide: &mut StreamOxide<inflate_state>) -> Result<MZStatus, MZError> {
    mz_inflate_init2_oxide(stream_oxide, MZ_DEFAULT_WINDOW_BITS)
}

pub fn mz_inflate_init2_oxide(stream_oxide: &mut StreamOxide<inflate_state>,
                              window_bits: c_int) -> Result<MZStatus, MZError>
{
    if (window_bits != MZ_DEFAULT_WINDOW_BITS) && (window_bits != -MZ_DEFAULT_WINDOW_BITS) {
        return Err(MZError::Param);
    }

    stream_oxide.adler = 0;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    stream_oxide.set_alloc_if_none(miniz_def_alloc_func);
    stream_oxide.set_free_if_none(miniz_def_free_func);

    unsafe {
        alloc_one!(stream_oxide, inflate_state)
    }.map_or(Err(MZError::Mem), |decompressor_state| {
        decompressor_state.m_decomp.m_state = 0;
        decompressor_state.m_dict_ofs = 0;
        decompressor_state.m_dict_avail = 0;
        decompressor_state.m_last_status = tinfl::TINFL_STATUS_NEEDS_MORE_INPUT;
        decompressor_state.m_first_call = 1;
        decompressor_state.m_has_flushed = 0;
        decompressor_state.m_window_bits = window_bits;
        stream_oxide.state = Some(decompressor_state);
        Ok(MZStatus::Ok)
    })
}

use tinfl::TINFL_STATUS_DONE;
use tinfl::TINFL_STATUS_FAILED;

fn push_dict_out(state: &mut inflate_state, next_out: &mut &mut [u8]) -> c_ulong {
    let n = cmp::min(state.m_dict_avail as usize, next_out.len());
    (next_out[..n]).copy_from_slice(&state.m_dict[state.m_dict_ofs as usize..state.m_dict_ofs as usize + n]);
    *next_out = &mut mem::replace(next_out, &mut [])[n..];
    state.m_dict_avail -= n as c_uint;
    state.m_dict_ofs = (state.m_dict_ofs + (n as c_uint)) & ((tinfl::TINFL_LZ_DICT_SIZE - 1) as c_uint);
    n as c_ulong
}

pub fn mz_inflate_oxide(stream_oxide: &mut StreamOxide<inflate_state>, mut flush: c_int) -> Result<MZStatus, MZError> {
    match stream_oxide.state {
        None => Err(MZError::Stream),
        Some(ref mut state) => {
            if flush == MZ_PARTIAL_FLUSH {
                flush = MZ_SYNC_FLUSH;
            }

            if (flush != MZ_NO_FLUSH) && (flush != MZ_SYNC_FLUSH) && (flush != MZ_FINISH) {
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

            if (state.m_has_flushed != 0) && (flush != MZ_FINISH) {
                return Err(MZError::Stream);
            }
            state.m_has_flushed |= (flush == MZ_FINISH) as c_uint;

            match (&mut stream_oxide.next_in, &mut stream_oxide.next_out) {
                (&mut Some(ref mut next_in), &mut Some(ref mut next_out)) => {
                    let orig_avail_in = next_in.len() as size_t;
                    if (flush == MZ_FINISH) && (first_call != 0) {
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

                    if flush != MZ_FINISH {
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

                    let mut status: c_int;
                    loop {
                        let mut in_bytes = next_in.len() as usize;
                        let mut out_bytes = tinfl::TINFL_LZ_DICT_SIZE - state.m_dict_ofs as usize;

                        status = unsafe { tinfl::tinfl_decompress(
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
                        } else if (status == tinfl::TINFL_STATUS_NEEDS_MORE_INPUT) && (orig_avail_in == 0) {
                            return Err(MZError::Buf);
                        } else if flush == MZ_FINISH {
                            if status == TINFL_STATUS_DONE {
                                return if state.m_dict_avail != 0 { Err(MZError::Buf) } else { Ok(MZStatus::StreamEnd) };
                            } else if next_out.len() == 0 {
                                return Err(MZError::Buf);
                            }
                        } else if (status == TINFL_STATUS_DONE) || (next_in.len() == 0) ||
                                  (next_out.len() == 0) || (state.m_dict_avail != 0) {
                            break;
                        }
                    }
                    if (status == TINFL_STATUS_DONE) && (state.m_dict_avail == 0) { Ok(MZStatus::StreamEnd) } else { Ok(MZStatus::Ok) }
                },
                _ => Err(MZError::Stream)
            }
        }
    }
}

pub fn mz_inflate_end_oxide(stream_oxide: &mut StreamOxide<inflate_state>) -> Result<MZStatus, MZError> {
    match stream_oxide.state {
        None => (),
        Some(ref mut state) => unsafe {
            free!(stream_oxide, (*state as *mut inflate_state) as *mut c_void);
        }
    };
    Ok(MZStatus::Ok)
}

// TODO: probably not covered by tests
pub fn mz_deflate_reset_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>) -> Result<MZStatus, MZError> {
    match (&mut stream_oxide.state, &stream_oxide.alloc, &stream_oxide.free) {
        (&mut Some(ref mut compressor_state), &Some(_), &Some(_)) => {
            stream_oxide.total_in = 0;
            stream_oxide.total_out = 0;
            unsafe {
                tdef::tdefl_init(*compressor_state, None, ptr::null_mut(), compressor_state.m_flags as c_int);
            }
            Ok(MZStatus::Ok)
        },
        _ => Err(MZError::Stream)
    }
}
