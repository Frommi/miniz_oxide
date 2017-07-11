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

pub struct StreamOxide<'s> {
    stream: &'s mut mz_stream,
}

impl<'s> StreamOxide<'s> {
    pub fn new(stream: &mut mz_stream) -> StreamOxide {
        StreamOxide { stream: stream }
    }

    pub fn as_mz_stream<'a>(&'a mut self) -> &'a mut mz_stream {
        self.stream
    }
}

pub fn mz_compress2_oxide(stream_oxide: &mut StreamOxide, level: c_int) -> c_int {
    let mut status: c_int = mz_deflate_init_oxide(stream_oxide, level);
    if status != MZ_OK {
        return status;
    }

    status = unsafe { mz_deflate(stream_oxide.as_mz_stream(), MZ_FINISH) };
    if status != MZ_STREAM_END {
        unsafe { mz_deflateEnd(stream_oxide.as_mz_stream()) };
        return if status == MZ_OK { MZ_BUF_ERROR } else { status };
    }

    unsafe { mz_deflateEnd(stream_oxide.as_mz_stream()) }
}


pub use tdef::TDEFL_COMPUTE_ADLER32;
pub use tdef::TDEFL_STATUS_OKAY;

pub fn mz_deflate_init_oxide(stream_oxide: &mut StreamOxide, level: c_int) -> c_int {
    mz_deflate_init2_oxide(stream_oxide, level, MZ_DEFLATED, MZ_DEFAULT_WINDOW_BITS, 9, MZ_DEFAULT_STRATEGY)
}

pub fn mz_deflate_init2_oxide(stream_oxide: &mut StreamOxide, level: c_int, method: c_int,
                              window_bits: c_int, mem_level: c_int, strategy: c_int) -> c_int {
    let comp_flags = TDEFL_COMPUTE_ADLER32 as u32 | unsafe { tdefl_create_comp_flags_from_zip_params(level, window_bits, strategy) };

    if (method != MZ_DEFLATED) || ((mem_level < 1) || (mem_level > 9)) ||
        ((window_bits != MZ_DEFAULT_WINDOW_BITS) && (-window_bits != MZ_DEFAULT_WINDOW_BITS)) {
        return MZ_PARAM_ERROR;
    }
    stream_oxide.as_mz_stream().data_type;
    stream_oxide.as_mz_stream().adler = MZ_ADLER32_INIT;
    stream_oxide.as_mz_stream().msg = ptr::null();
    stream_oxide.as_mz_stream().reserved = 0;
    stream_oxide.as_mz_stream().total_in = 0;
    stream_oxide.as_mz_stream().total_out = 0;

    if stream_oxide.as_mz_stream().zalloc.is_none() {
        stream_oxide.as_mz_stream().zalloc = Some(miniz_def_alloc_func);
    }
    if stream_oxide.as_mz_stream().zfree.is_none() {
        stream_oxide.as_mz_stream().zfree = Some(miniz_def_free_func);
    }

    let comp = unsafe {
        stream_oxide.as_mz_stream().zalloc.unwrap()(
            stream_oxide.as_mz_stream().opaque,
            1,
            mem::size_of::<tdefl_compressor>()
        ) as *mut tdefl_compressor
    };

    if comp.is_null() {
        return MZ_MEM_ERROR;
    }

    stream_oxide.as_mz_stream().state = comp as *mut mz_internal_state;
    if unsafe { tdefl_init(comp, None, ptr::null_mut(), comp_flags as c_int) } != TDEFL_STATUS_OKAY {
        unsafe { mz_deflateEnd(stream_oxide.as_mz_stream()) };
        return MZ_PARAM_ERROR;
    }

    MZ_OK
}
