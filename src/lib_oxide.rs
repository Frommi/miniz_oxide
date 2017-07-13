use super::*;

pub fn mz_adler32_oxide(adler: c_ulong, data: &[u8]) -> c_ulong {
    let mut s1 = adler & 0xffff;
    let mut s2 = adler >> 16;
    for x in data {
        s1 = (s1 + *x as c_ulong) % 65521;
        s2 = (s1 + s2) % 65521;
    }
    (s2 << 16) + s1
}

#[allow(bad_style)]
pub struct tinfl_decompressor_mock();

pub trait StateType {}
impl StateType for tdefl_compressor {}
impl StateType for tinfl_decompressor_mock {}

pub enum StateOxide<'a> {
    Compressor(&'a mut tdefl_compressor),
    Decompressor(&'a mut tinfl_decompressor_mock)
}

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
            total_in: 0,
            next_out: out_slice,
            total_out: 0,
            state: None,
            alloc: None,
            free: None,
            opaque: ptr::null_mut(),
            adler: 0
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


pub fn mz_compress2_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>, level: c_int, dest_len: &mut c_ulong) -> c_int {
    let mut status: c_int = mz_deflate_init_oxide(stream_oxide, level);
    if status != MZ_OK {
        return status;
    }

    let mut stream = stream_oxide.as_mz_stream();
    status = unsafe { mz_deflate(&mut stream, MZ_FINISH) };
    if status != MZ_STREAM_END {
        unsafe { mz_deflateEnd(&mut stream) };
        return if status == MZ_OK { MZ_BUF_ERROR } else { status };
    }

    let res = unsafe { mz_deflateEnd(&mut stream) };
    *stream_oxide = unsafe { StreamOxide::new(&mut stream) };
    *dest_len = stream_oxide.total_out;
    res
}


pub use tdef::TDEFL_COMPUTE_ADLER32;
pub use tdef::TDEFL_STATUS_OKAY;

pub fn mz_deflate_init_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>, level: c_int) -> c_int {
    mz_deflate_init2_oxide(stream_oxide, level, MZ_DEFLATED, MZ_DEFAULT_WINDOW_BITS, 9, MZ_DEFAULT_STRATEGY)
}

pub fn mz_deflate_init2_oxide(stream_oxide: &mut StreamOxide<tdefl_compressor>, level: c_int, method: c_int,
                              window_bits: c_int, mem_level: c_int, strategy: c_int) -> c_int {
    let comp_flags = TDEFL_COMPUTE_ADLER32 as u32 |
        unsafe {
            tdefl_create_comp_flags_from_zip_params(level, window_bits, strategy)
        };

    if (method != MZ_DEFLATED) || ((mem_level < 1) || (mem_level > 9)) ||
        ((window_bits != MZ_DEFAULT_WINDOW_BITS) && (-window_bits != MZ_DEFAULT_WINDOW_BITS)) {
        return MZ_PARAM_ERROR;
    }

    stream_oxide.adler = MZ_ADLER32_INIT;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    stream_oxide.set_alloc_if_none(miniz_def_alloc_func);
    stream_oxide.set_free_if_none(miniz_def_free_func);

    match unsafe { alloc_one!(stream_oxide, tdefl_compressor) } {
        None => MZ_MEM_ERROR,
        Some(compressor_state) => {
            if unsafe { tdefl_init(compressor_state, None, ptr::null_mut(), comp_flags as c_int) } != TDEFL_STATUS_OKAY {
                let mut stream = stream_oxide.as_mz_stream();
                unsafe { mz_deflateEnd(&mut stream) };
                *stream_oxide = unsafe { StreamOxide::new(&mut stream) };
                return MZ_PARAM_ERROR;
            }
            stream_oxide.state = Some(compressor_state);
            MZ_OK
        }
    }
}
