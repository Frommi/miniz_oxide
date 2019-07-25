//! This module mainly contains functionality replicating the miniz higher level API.

use std::{cmp, mem, usize};
use std::io::Cursor;
use std::default::Default;

use miniz_oxide::deflate::core::{CompressorOxide, CompressionStrategy, TDEFLFlush, TDEFLStatus,
              compress, create_comp_flags_from_zip_params, deflate_flags};
use tdef::Compressor;
use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::{TINFL_LZ_DICT_SIZE, inflate_flags, DecompressorOxide};

use miniz_oxide::*;

const MZ_DEFLATED: i32 = 8;
const MZ_DEFAULT_WINDOW_BITS: i32 = 15;

pub const MZ_ADLER32_INIT: u32 = 1;

pub enum InternalState {
    Inflate(Box<InflateState>),
    Deflate(Box<Compressor>),
}

pub type MZResult = Result<MZStatus, MZError>;

/// Enum to keep track of what type the internal state is when moving over the C API boundary.
#[repr(C)]
#[derive(Debug,Copy,Clone,PartialEq)]
pub enum StateTypeEnum {
    None = 0,
    Inflate,
    Deflate,
}

/// Trait used for states that can be carried by BoxedState.
pub trait StateType {
    const STATE_TYPE: StateTypeEnum;
    fn from_enum(&mut InternalState) -> Option<&mut Self>;
}

impl StateType for InflateState {
    const STATE_TYPE: StateTypeEnum = StateTypeEnum::Inflate;
    fn from_enum(value: &mut InternalState) -> Option<&mut Self> {
        if let InternalState::Inflate(state) = value {
            Some(state.as_mut())
        } else {
            None
        }
    }
}

impl StateType for Compressor {
    const STATE_TYPE: StateTypeEnum = StateTypeEnum::Deflate;
    fn from_enum(value: &mut InternalState) -> Option<&mut Self> {
        if let InternalState::Deflate(state) = value {
            Some(state.as_mut())
        } else {
            None
        }
    }
}

pub struct StreamOxide<'io, ST: StateType> {
    pub next_in: Option<&'io [u8]>,
    pub total_in: u64,

    pub next_out: Option<&'io mut [u8]>,
    pub total_out: u64,

    pub(crate) state: Option<Box<InternalState>>,

    pub adler: u32,
    pub(crate) state_type: std::marker::PhantomData<ST>,
}

impl<'io, ST: StateType> StreamOxide<'io, ST> {
    pub fn state(&mut self) -> Option<&mut ST> {
        StateType::from_enum(self.state.as_mut()?.as_mut())
    }
}

/// Returns true if the window_bits parameter is valid.
fn invalid_window_bits(window_bits: i32) -> bool {
    (window_bits != MZ_DEFAULT_WINDOW_BITS) && (-window_bits != MZ_DEFAULT_WINDOW_BITS)
}

/// Try to fully decompress the data provided in the stream struct, with the specified
/// level.
///
/// Returns MZResult::Ok on success.
pub fn mz_compress2_oxide(
    stream_oxide: &mut StreamOxide<Compressor>,
    level: i32,
    dest_len: &mut u64,
) -> MZResult {
    mz_deflate_init_oxide(stream_oxide, level)?;
    let status = mz_deflate_oxide(stream_oxide, MZFlush::Finish as i32);
    mz_deflate_end_oxide(stream_oxide)?;

    match status {
        Ok(MZStatus::StreamEnd) => {
            *dest_len = stream_oxide.total_out;
            Ok(MZStatus::Ok)
        }
        Ok(MZStatus::Ok) => Err(MZError::Buf),
        _ => status,
    }
}


/// Initialize the wrapped compressor with the requested level (0-10) and default settings.
///
/// The compression level will be set to 6 (default) if the requested level is not available.
pub fn mz_deflate_init_oxide(
    stream_oxide: &mut StreamOxide<Compressor>,
    level: i32,
) -> MZResult {
    mz_deflate_init2_oxide(
        stream_oxide,
        level,
        MZ_DEFLATED,
        MZ_DEFAULT_WINDOW_BITS,
        9,
        CompressionStrategy::Default as i32,
    )
}

/// Initialize the compressor with the requested parameters.
///
/// # Params
/// stream_oxide: The stream to be initialized.
/// level: Compression level (0-10).
/// method: Compression method. Only `MZ_DEFLATED` is accepted.
/// window_bits: Number of bits used to represent the compression sliding window.
///              Only `MZ_DEFAULT_WINDOW_BITS` is currently supported.
///              A negative value, i.e `-MZ_DEFAULT_WINDOW_BITS` indicates that the stream
///              should be wrapped in a zlib wrapper.
/// mem_level: Currently unused. Only values from 1 to and including 9 are accepted.
/// strategy: Compression strategy. See `deflate::CompressionStrategy` for accepted options.
///           The default, which is used in most cases, is 0.
pub fn mz_deflate_init2_oxide(
    stream_oxide: &mut StreamOxide<Compressor>,
    level: i32,
    method: i32,
    window_bits: i32,
    mem_level: i32,
    strategy: i32,
) -> MZResult {
    let comp_flags = deflate_flags::TDEFL_COMPUTE_ADLER32 |
        create_comp_flags_from_zip_params(level, window_bits, strategy);

    let invalid_level = (mem_level < 1) || (mem_level > 9);
    if (method != MZ_DEFLATED) || invalid_level || invalid_window_bits(window_bits) {
        return Err(MZError::Param);
    }


    stream_oxide.adler = MZ_ADLER32_INIT;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    let mut compr: Box<Compressor> = Box::default();
    compr.inner = Some(CompressorOxide::new(comp_flags));
    stream_oxide.state = Some(Box::new(InternalState::Deflate(compr)));

    Ok(MZStatus::Ok)
}

pub fn mz_deflate_oxide(
    stream_oxide: &mut StreamOxide<Compressor>,
    flush: i32,
) -> MZResult {
    let state: &mut Compressor = {
        let enum_ref = stream_oxide.state.as_mut().ok_or(MZError::Stream)?;
        StateType::from_enum(enum_ref)
    }.ok_or(MZError::Stream)?;
    let next_in = stream_oxide.next_in.as_mut().ok_or(MZError::Stream)?;
    let next_out = stream_oxide.next_out.as_mut().ok_or(MZError::Stream)?;

    let flush = MZFlush::new(flush)?;

    if next_out.is_empty() {
        return Err(MZError::Buf);
    }

    if state.prev_return_status() == TDEFLStatus::Done {
        return if flush == MZFlush::Finish {
            Ok(MZStatus::StreamEnd)
        } else {
            Err(MZError::Buf)
        };
    }

    let original_total_in = stream_oxide.total_in;
    let original_total_out = stream_oxide.total_out;

    if let Some(compressor) = state.inner.as_mut() {

    loop {
        let in_bytes;
        let out_bytes;
        let defl_status = {
            let res = compress(compressor, *next_in, *next_out, TDEFLFlush::from(flush));
            in_bytes = res.1;
            out_bytes = res.2;
            res.0
        };

        *next_in = &next_in[in_bytes..];
        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
        stream_oxide.total_in += in_bytes as u64;
        stream_oxide.total_out += out_bytes as u64;
        stream_oxide.adler = compressor.adler32();

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
            };
        }
    }
    } else {
        Err(MZError::Param)
    }
}

/// Free the inner compression state.
///
/// Currently always returns `MZStatus::Ok`.
pub fn mz_deflate_end_oxide(stream_oxide: &mut StreamOxide<Compressor>) -> MZResult {
    stream_oxide.state = None;
    Ok(MZStatus::Ok)
}


/// Reset the compressor, so it can be used to compress a new set of data.
///
/// Returns `MZError::Stream` if the inner stream is missing, otherwise `MZStatus::Ok`.
// TODO: probably not covered by tests
pub fn mz_deflate_reset_oxide(stream_oxide: &mut StreamOxide<Compressor>) -> MZResult {
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;
    let state = stream_oxide.state().ok_or(MZError::Stream)?;
    state.drop_inner();
    *state = Compressor::new(state.flags());
    Ok(MZStatus::Ok)
}



#[repr(C)]
#[allow(bad_style)]
pub struct InflateState {
    pub m_decomp: DecompressorOxide,

    pub m_dict_ofs: usize,
    pub m_dict_avail: usize,
    pub m_first_call: u32,
    pub m_has_flushed: u32,

    pub m_window_bits: i32,
    pub m_dict: [u8; TINFL_LZ_DICT_SIZE],
    pub m_last_status: TINFLStatus,
}

impl Default for InflateState {
    fn default() -> Self {
        InflateState {
            m_decomp: DecompressorOxide::default(),
            m_dict_ofs: 0,
            m_dict_avail: 0,
            m_first_call: 1,
            m_has_flushed: 0,

            m_window_bits: MZ_DEFAULT_WINDOW_BITS,
            m_dict: [0; TINFL_LZ_DICT_SIZE],
            m_last_status: TINFLStatus::NeedsMoreInput,
        }
    }
}
impl InflateState {
    fn new_boxed(window_bits: i32) -> Box<InflateState> {
        let mut b: Box<InflateState> = Box::default();
        b.m_window_bits = window_bits;
        b
    }
}

pub fn mz_inflate_init_oxide(stream_oxide: &mut StreamOxide<InflateState>) -> MZResult {
    mz_inflate_init2_oxide(stream_oxide, MZ_DEFAULT_WINDOW_BITS)
}

pub fn mz_inflate_init2_oxide(
    stream_oxide: &mut StreamOxide<InflateState>,
    window_bits: i32,
) -> MZResult {
    if invalid_window_bits(window_bits) {
        return Err(MZError::Param);
    }

    stream_oxide.adler = 0;
    stream_oxide.total_in = 0;
    stream_oxide.total_out = 0;

    stream_oxide.state = Some(Box::new(InternalState::Inflate(
        InflateState::new_boxed(window_bits))));

    let state = stream_oxide.state().ok_or(MZError::Mem)?;
    state.m_decomp.init();
    state.m_dict_ofs = 0;
    state.m_dict_avail = 0;
    state.m_last_status = TINFLStatus::NeedsMoreInput;
    state.m_first_call = 1;
    state.m_has_flushed = 0;
    state.m_window_bits = window_bits;

    Ok(MZStatus::Ok)
}

fn push_dict_out(state: &mut InflateState, next_out: &mut &mut [u8]) -> usize {
    let n = cmp::min(state.m_dict_avail as usize, next_out.len());
    (next_out[..n]).copy_from_slice(
        &state.m_dict[state.m_dict_ofs..state.m_dict_ofs + n],
    );
    *next_out = &mut mem::replace(next_out, &mut [])[n..];
    state.m_dict_avail -= n;
    state.m_dict_ofs = (state.m_dict_ofs + (n)) &
        ((TINFL_LZ_DICT_SIZE - 1));
    n
}

pub fn mz_inflate_oxide(stream_oxide: &mut StreamOxide<InflateState>, flush: i32) -> MZResult {
    let state: &mut InflateState = {
        let enum_ref = stream_oxide.state.as_mut().ok_or(MZError::Stream)?;
        StateType::from_enum(enum_ref)
    }.ok_or(MZError::Stream)?;


    let next_in = stream_oxide.next_in.as_mut().ok_or(MZError::Stream)?;
    let next_out = stream_oxide.next_out.as_mut().ok_or(MZError::Stream)?;

    let flush = MZFlush::new(flush)?;
    if flush == MZFlush::Full {
        return Err(MZError::Stream);
    }

    let mut decomp_flags = inflate_flags::TINFL_FLAG_COMPUTE_ADLER32;
    if state.m_window_bits > 0 {
        decomp_flags |= inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER;
    }

    let first_call = state.m_first_call;
    state.m_first_call = 0;
    if (state.m_last_status as i32) < 0 {
        return Err(MZError::Data);
    }

    if (state.m_has_flushed != 0) && (flush != MZFlush::Finish) {
        return Err(MZError::Stream);
    }
    state.m_has_flushed |= (flush == MZFlush::Finish) as u32;

    let orig_avail_in = next_in.len();

    if (flush == MZFlush::Finish) && (first_call != 0) {
        decomp_flags |= inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
        let status = inflate::core::decompress(
            &mut state.m_decomp,
            *next_in,
            &mut Cursor::new(*next_out),
            decomp_flags,
        );
        let in_bytes = status.1;
        let out_bytes = status.2;
        let status = status.0;

        state.m_last_status = status;

        *next_in = &next_in[in_bytes..];
        *next_out = &mut mem::replace(next_out, &mut [])[out_bytes..];
        stream_oxide.total_in += in_bytes as u64;
        stream_oxide.total_out += out_bytes as u64;
        // Simply set this to 0 if it doesn't exist.
        stream_oxide.adler = state.m_decomp.adler32().unwrap_or(0).into();

        if (status as i32) < 0 {
            return Err(MZError::Data);
        } else if status != TINFLStatus::Done {
            state.m_last_status = TINFLStatus::Failed;
            return Err(MZError::Buf);
        }
        return Ok(MZStatus::StreamEnd);
    }

    if flush != MZFlush::Finish {
        decomp_flags |= inflate_flags::TINFL_FLAG_HAS_MORE_INPUT;
    }

    if state.m_dict_avail != 0 {
        stream_oxide.total_out += push_dict_out(state, next_out) as u64;
        return if (state.m_last_status == TINFLStatus::Done) &&
            (state.m_dict_avail == 0)
            {
                Ok(MZStatus::StreamEnd)
            } else {
            Ok(MZStatus::Ok)
        };
    }

    loop {
        let status = {
            let mut out_cursor = Cursor::new(&mut state.m_dict[..]);
            out_cursor.set_position(state.m_dict_ofs as u64);
            inflate::core::decompress(&mut state.m_decomp, *next_in, &mut out_cursor, decomp_flags)
        };

        let in_bytes = status.1;
        let out_bytes = status.2;
        let status = status.0;

        state.m_last_status = status;

        *next_in = &next_in[in_bytes..];
        stream_oxide.total_in += in_bytes as u64;

        state.m_dict_avail = out_bytes;
        stream_oxide.total_out += push_dict_out(state, next_out) as u64;
        stream_oxide.adler = state.m_decomp.adler32().unwrap_or(0).into();

        if (status as i32) < 0 {
            return Err(MZError::Data);
        }

        if (status == TINFLStatus::NeedsMoreInput) && (orig_avail_in == 0) {
            return Err(MZError::Buf);
        }

        if flush == MZFlush::Finish {
            if status == TINFLStatus::Done {
                return if state.m_dict_avail != 0 {
                    Err(MZError::Buf)
                } else {
                    Ok(MZStatus::StreamEnd)
                };
            } else if next_out.is_empty() {
                return Err(MZError::Buf);
            }
        } else {
            let empty_buf = next_in.is_empty() || next_out.is_empty();
            if (status == TINFLStatus::Done) || empty_buf || (state.m_dict_avail != 0) {
                return if (status == TINFLStatus::Done) && (state.m_dict_avail == 0) {
                    Ok(MZStatus::StreamEnd)
                } else {
                    Ok(MZStatus::Ok)
                };
            }
        }
    }
}

pub fn mz_uncompress2_oxide(
    stream_oxide: &mut StreamOxide<InflateState>,
    dest_len: &mut u64,
) -> MZResult {
    mz_inflate_init_oxide(stream_oxide)?;
    let status = mz_inflate_oxide(stream_oxide, MZFlush::Finish as i32);
    mz_inflate_end_oxide(stream_oxide)?;

    let empty_in = stream_oxide.next_in.map_or(
        true,
        |next_in| next_in.is_empty(),
    );
    match (status, empty_in) {
        (Ok(MZStatus::StreamEnd), _) => {
            *dest_len = stream_oxide.total_out;
            Ok(MZStatus::Ok)
        }
        (Err(MZError::Buf), true) => Err(MZError::Data),
        (status, _) => status,
    }
}

pub fn mz_inflate_end_oxide(stream_oxide: &mut StreamOxide<InflateState>) -> MZResult {
    stream_oxide.state = None;
    Ok(MZStatus::Ok)
}
