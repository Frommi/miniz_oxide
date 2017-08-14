use super::*;

pub fn memset<T : Clone>(slice: &mut [T], val: T) {
    for x in slice { *x = val.clone() }
}

pub struct CompressorOxide {
    pub lz: LZOxide,
    pub params: ParamsOxide,
    pub callback: Option<CallbackFunc>,
    pub local_buf: [u8; TDEFL_OUT_BUF_SIZE],
    pub huff: HuffmanOxide,
    pub dict: DictOxide,
}

#[derive(Copy, Clone)]
pub struct CallbackFunc {
    pub put_buf_func: PutBufFuncPtrNotNull,
    pub put_buf_user: *mut c_void,
}

pub struct CallbackBuf<'a> {
    pub out_buf: &'a mut [u8],
}

pub enum CallbackOut<'a> {
    Func(CallbackFunc),
    Buf(CallbackBuf<'a>),
}

impl<'a> CallbackOut<'a> {
    pub fn new_output_buffer<'b>(
        &'b mut self,
        local_buf: &'b mut [u8],
        out_buf_ofs: usize
    ) -> OutputBufferOxide<'b> {
        let is_local;
        let buf_len = TDEFL_OUT_BUF_SIZE - 16;
        let chosen_buffer = match *self {
            CallbackOut::Buf(ref mut cb) if cb.out_buf.len() - out_buf_ofs >= TDEFL_OUT_BUF_SIZE => {
                is_local = false;
                &mut cb.out_buf[out_buf_ofs..out_buf_ofs + buf_len]
            },
            _ => {
                is_local = true;
                &mut local_buf[..buf_len]
            },
        };

        let cursor = Cursor::new(chosen_buffer);
        OutputBufferOxide {
            inner: cursor,
            local: is_local,
            bit_buffer: 0,
            bits_in: 0,
        }
    }
}

pub struct CallbackOxide<'a> {
    pub in_buf: Option<&'a [u8]>,
    pub out: CallbackOut<'a>,
}

impl<'a> CallbackOxide<'a> {
    pub unsafe fn new(
        callback_func: Option<CallbackFunc>,
        in_buf: *const c_void,
        in_size: usize,
        out_buf: *mut c_void,
        out_size: usize,
    ) -> Result<Self, TDEFLStatus> {
        let out = match callback_func {
            None => CallbackOut::Buf(CallbackBuf {
                out_buf: slice::from_raw_parts_mut(
                    (out_buf as *mut u8).as_mut().ok_or(TDEFLStatus::BadParam)?,
                    out_size
                ),
            }),
            Some(func) => {
                if out_size > 0 || out_buf.as_mut().is_some() {
                    return Err(TDEFLStatus::BadParam);
                }
                CallbackOut::Func(func)
            },
        };

        if in_size > 0 && in_buf.is_null() {
            return Err(TDEFLStatus::BadParam);
        }

        Ok(CallbackOxide {
            in_buf: (in_buf as *const u8).as_ref().map(|in_buf|
                slice::from_raw_parts(in_buf, in_size)
            ),
            out: out,
        })
    }
}

impl CompressorOxide {
    pub unsafe fn new(callback: Option<CallbackFunc>, flags: c_uint) -> Self {
        CompressorOxide {
            callback: callback,
            lz: LZOxide::new(),
            params: ParamsOxide::new(flags),
            local_buf: [0; TDEFL_OUT_BUF_SIZE],
            huff: HuffmanOxide::new(),
            dict: DictOxide::new(flags),
        }
    }
}

pub struct HuffmanOxide {
    pub count: [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub codes: [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub code_sizes: [[u8; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
}

impl HuffmanOxide {
    pub fn new() -> Self {
        HuffmanOxide {
            count: [[0; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
            codes: [[0; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
            code_sizes: [[0; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
        }
    }
}

pub struct OutputBufferOxide<'a> {
    pub inner: Cursor<&'a mut [u8]>,
    pub local: bool,

    pub bit_buffer: u32,
    pub bits_in: u32,
}

pub struct BitBuffer {
    pub bit_buffer: u64,
    pub bits_in: u32,
}

impl BitBuffer {
    pub fn put_fast(&mut self, bits: u64, len: u32) {
        self.bit_buffer |= bits << self.bits_in;
        self.bits_in += len;
    }

    pub fn flush(&mut self, output: &mut OutputBufferOxide) -> io::Result<()> {
        let pos = output.inner.position() as usize;
        let inner = &mut((*output.inner.get_mut())[pos]) as *mut u8 as *mut u64;
        unsafe {
            ptr::write_unaligned(inner, self.bit_buffer);
        }
        output.inner.seek(SeekFrom::Current((self.bits_in >> 3) as i64))?;
        self.bit_buffer >>= self.bits_in & !7;
        self.bits_in &= 7;
        Ok(())
    }
}

pub struct DictOxide {
    pub max_probes: [c_uint; 2],
    pub dict: [u8; TDEFL_LZ_DICT_SIZE + TDEFL_MAX_MATCH_LEN - 1],
    pub next: [u16; TDEFL_LZ_DICT_SIZE],
    pub hash: [u16; TDEFL_LZ_DICT_SIZE],

    pub code_buf_dict_pos: c_uint,
    pub lookahead_size: c_uint,
    pub lookahead_pos: c_uint,
    pub size: c_uint,
}

impl DictOxide {
    pub fn new(flags: c_uint) -> Self {
        DictOxide {
            max_probes: [
                1 + ((flags & 0xFFF) + 2) / 3,
                1 + (((flags & 0xFFF) >> 2) + 2) / 3
            ],
            dict: [0; TDEFL_LZ_DICT_SIZE + TDEFL_MAX_MATCH_LEN - 1],
            next: [0; TDEFL_LZ_DICT_SIZE],
            hash: [0; TDEFL_LZ_DICT_SIZE],

            code_buf_dict_pos: 0,
            lookahead_size: 0,
            lookahead_pos: 0,
            size: 0,
        }
    }
}

pub struct LZOxide {
    pub codes: [u8; TDEFL_LZ_CODE_BUF_SIZE],
    pub code_position: usize,
    pub flag_position: usize,

    pub total_bytes: c_uint,
    pub num_flags_left: c_uint,
}

pub struct ParamsOxide {
    pub flags: c_uint,
    pub greedy_parsing: bool,
    pub block_index: c_uint,

    pub saved_match_dist: c_uint,
    pub saved_match_len: libc::c_uint,
    pub saved_lit: u8,

    pub flush: TDEFLFlush,
    pub flush_ofs: c_uint,
    pub flush_remaining: c_uint,
    pub finished: bool,

    pub adler32: c_uint,

    pub src_pos: usize,
    pub src_buf_left: usize,

    pub out_buf_ofs: usize,
    pub prev_return_status: TDEFLStatus,

    pub saved_bit_buffer: u32,
    pub saved_bits_in: u32,
}

impl ParamsOxide {
    pub fn new(flags: c_uint) -> Self {
        ParamsOxide {
            flags: flags,
            greedy_parsing: flags & TDEFL_GREEDY_PARSING_FLAG != 0,
            block_index: 0,
            saved_match_dist: 0,
            saved_match_len: 0,
            saved_lit: 0,
            flush: TDEFLFlush::None,
            flush_ofs: 0,
            flush_remaining: 0,
            finished: false,
            adler32: ::MZ_ADLER32_INIT as c_uint,
            src_pos: 0,
            src_buf_left: 0,
            out_buf_ofs: 0,
            prev_return_status: TDEFLStatus::Okay,
            saved_bit_buffer: 0,
            saved_bits_in: 0,
        }
    }
}

impl LZOxide {
    pub fn new() -> Self {
        LZOxide {
            codes: [0; TDEFL_LZ_CODE_BUF_SIZE],
            code_position: 1,
            flag_position: 0,
            total_bytes: 0,
            num_flags_left: 8,
        }
    }

    pub fn write_code(&mut self, val: u8) {
        self.codes[self.code_position] = val;
        self.code_position += 1;
    }

    pub fn init_flag(&mut self) {
        if self.num_flags_left == 8 {
            *self.get_flag() = 0;
            self.code_position -= 1;
        } else {
            *self.get_flag() >>= self.num_flags_left;
        }
    }

    pub fn get_flag(&mut self) -> &mut u8 {
        &mut self.codes[self.flag_position]
    }

    pub fn plant_flag(&mut self) {
        self.flag_position = self.code_position;
        self.code_position += 1;
    }

    pub fn consume_flag(&mut self) {
        self.num_flags_left -= 1;
        if self.num_flags_left == 0 {
            self.num_flags_left = 8;
            self.plant_flag();
        }
    }
}

pub struct SavedOutputBufferOxide {
    pub pos: u64,
    pub bit_buffer: u32,
    pub bits_in: u32,
    pub local: bool,
}

impl<'a> OutputBufferOxide<'a> {
    fn put_bits(&mut self, bits: u32, len: u32) -> io::Result<()> {
        assert!(bits <= ((1u32 << len) - 1u32));
        self.bit_buffer |= bits << self.bits_in;
        self.bits_in += len;
        while self.bits_in >= 8 {
            self.inner.write(&[self.bit_buffer as u8][..])?;
            self.bit_buffer >>= 8;
            self.bits_in -= 8;
        }
        Ok(())
    }

    fn save(&self) -> SavedOutputBufferOxide {
        SavedOutputBufferOxide {
            pos: self.inner.position(),
            bit_buffer: self.bit_buffer,
            bits_in: self.bits_in,
            local: self.local,
        }
    }

    fn load(&mut self, saved: SavedOutputBufferOxide) {
        self.inner.set_position(saved.pos);
        self.bit_buffer = saved.bit_buffer;
        self.bits_in = saved.bits_in;
        self.local = saved.local;
    }

    fn load_bits(&mut self, saved: &SavedOutputBufferOxide) {
        self.bit_buffer = saved.bit_buffer;
        self.bits_in = saved.bits_in;
    }

    fn pad_to_bytes(&mut self) -> io::Result<()> {
        if self.bits_in != 0 {
            let len = 8 - self.bits_in;
            self.put_bits(0, len)?;
        }

        Ok(())
    }
}

pub fn tdefl_radix_sort_syms_oxide<'a>(
    symbols0: &'a mut [tdefl_sym_freq],
    symbols1: &'a mut [tdefl_sym_freq]
) -> &'a mut [tdefl_sym_freq] {
    let mut hist = [[0; 256]; 2];

    for freq in symbols0.iter() {
        hist[0][(freq.m_key & 0xFF) as usize] += 1;
        hist[1][((freq.m_key >> 8) & 0xFF) as usize] += 1;
    }

    let mut n_passes = 2;
    if symbols0.len() == hist[1][0] {
        n_passes -= 1;
    }

    let mut current_symbols = symbols0;
    let mut new_symbols = symbols1;

    for pass in 0..n_passes {
        let mut offsets = [0; 256];
        let mut offset = 0;
        for i in 0..256 {
            offsets[i] = offset;
            offset += hist[pass][i];
        }

        for sym in current_symbols.iter() {
            let j = ((sym.m_key >> (pass * 8)) & 0xFF) as usize;
            new_symbols[offsets[j]] = *sym;
            offsets[j] += 1;
        }

        mem::swap(&mut current_symbols, &mut new_symbols);
    }

    current_symbols
}

pub fn tdefl_calculate_minimum_redundancy_oxide(symbols: &mut [tdefl_sym_freq]) {
    match symbols.len() {
        0 => (),
        1 => symbols[0].m_key = 1,
        n => {
            symbols[0].m_key += symbols[1].m_key;
            let mut root = 0;
            let mut leaf = 2;
            for next in 1..n - 1 {
                if (leaf >= n) || (symbols[root].m_key < symbols[leaf].m_key) {
                    symbols[next].m_key = symbols[root].m_key;
                    symbols[root].m_key = next as u16;
                    root += 1;
                } else {
                    symbols[next].m_key = symbols[leaf].m_key;
                    leaf += 1;
                }

                if (leaf >= n) || (root < next && symbols[root].m_key < symbols[leaf].m_key) {
                    symbols[next].m_key = symbols[next].m_key.wrapping_add(symbols[root].m_key);
                    symbols[root].m_key = next as u16;
                    root += 1;
                } else {
                    symbols[next].m_key = symbols[next].m_key.wrapping_add(symbols[leaf].m_key);
                    leaf += 1;
                }
            }

            symbols[n - 2].m_key = 0;
            for next in (0..n - 2).rev() {
                symbols[next].m_key = symbols[symbols[next].m_key as usize].m_key + 1;
            }

            let mut avbl = 1;
            let mut used = 0;
            let mut dpth = 0;
            let mut root = (n - 2) as i32;
            let mut next = (n - 1) as i32;
            while avbl > 0 {
                while (root >= 0) && (symbols[root as usize].m_key == dpth) {
                    used += 1;
                    root -= 1;
                }
                while avbl > used {
                    symbols[next as usize].m_key = dpth;
                    next -= 1;
                    avbl -= 1;
                }
                avbl = 2 * used;
                dpth += 1;
                used = 0;
            }
        }
    }
}

pub fn tdefl_huffman_enforce_max_code_size_oxide(
    num_codes: &mut [c_int],
    code_list_len: usize,
    max_code_size: usize
) {
    if code_list_len <= 1 { return; }

    num_codes[max_code_size] += num_codes[max_code_size + 1..].iter().sum();
    let total = num_codes[1..max_code_size + 1].iter().rev().enumerate().fold(0u32, |total, (i, &x)| {
        total + ((x as u32) << i)
    });

    for _ in (1 << max_code_size)..total {
        num_codes[max_code_size] -= 1;
        for i in (1..max_code_size).rev() {
            if num_codes[i] != 0 {
                num_codes[i] -= 1;
                num_codes[i + 1] += 2;
                break;
            }
        }
    }
}

pub fn tdefl_optimize_huffman_table_oxide(
    h: &mut HuffmanOxide,
    table_num: usize,
    table_len: usize,
    code_size_limit: usize,
    static_table: bool
) {
    let mut num_codes = [0 as c_int; TDEFL_MAX_SUPPORTED_HUFF_CODESIZE + 1];
    let mut next_code = [0 as c_uint; TDEFL_MAX_SUPPORTED_HUFF_CODESIZE + 1];

    if static_table {
        for &code_size in &h.code_sizes[table_num][..table_len] {
            num_codes[code_size as usize] += 1;
        }
    } else {
        let mut symbols0 = [tdefl_sym_freq { m_key: 0, m_sym_index: 0 }; TDEFL_MAX_HUFF_SYMBOLS];
        let mut symbols1 = [tdefl_sym_freq { m_key: 0, m_sym_index: 0 }; TDEFL_MAX_HUFF_SYMBOLS];

        let mut num_used_symbols = 0;
        for i in 0..table_len {
            if h.count[table_num][i] != 0 {
                symbols0[num_used_symbols] = tdefl_sym_freq {
                    m_key: h.count[table_num][i],
                    m_sym_index: i as u16
                };
                num_used_symbols += 1;
            }
        }

        let mut symbols = tdefl_radix_sort_syms_oxide(&mut symbols0[..num_used_symbols],
                                                      &mut symbols1[..num_used_symbols]);
        tdefl_calculate_minimum_redundancy_oxide(symbols);

        for symbol in symbols.iter() {
            num_codes[symbol.m_key as usize] += 1;
        }

        tdefl_huffman_enforce_max_code_size_oxide(&mut num_codes, num_used_symbols, code_size_limit);

        memset(&mut h.code_sizes[table_num][..], 0);
        memset(&mut h.codes[table_num][..], 0);

        let mut last = num_used_symbols;
        for i in 1..code_size_limit + 1 {
            let first = last - num_codes[i] as usize;
            for symbol in &symbols[first..last] {
                h.code_sizes[table_num][symbol.m_sym_index as usize] = i as u8;
            }
            last = first;
        }
    }

    let mut j = 0;
    next_code[1] = 0;
    for i in 2..code_size_limit + 1 {
        j = (j + num_codes[i - 1]) << 1;
        next_code[i] = j as c_uint;
    }

    for (&code_size, huff_code) in h.code_sizes[table_num].iter().take(table_len)
                                    .zip(h.codes[table_num].iter_mut().take(table_len))
    {
        if code_size == 0 { continue }

        let mut code = next_code[code_size as usize];
        next_code[code_size as usize] += 1;

        let mut rev_code = 0;
        for _ in 0..code_size { // TODO reverse u32 faster?
            rev_code = (rev_code << 1) | (code & 1);
            code >>= 1;
        }
        *huff_code = rev_code as u16;
    }
}

const TDEFL_PACKED_CODE_SIZE_SYMS_SWIZZLE: [u8; 19] =
    [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

pub fn tdefl_start_dynamic_block_oxide(
    h: &mut HuffmanOxide,
    output: &mut OutputBufferOxide
) -> io::Result<()> {
    h.count[0][256] = 1;

    tdefl_optimize_huffman_table_oxide(h, 0, TDEFL_MAX_HUFF_SYMBOLS_0, 15, false);
    tdefl_optimize_huffman_table_oxide(h, 1, TDEFL_MAX_HUFF_SYMBOLS_1, 15, false);

    let num_lit_codes = 286 - &h.code_sizes[0][257..286]
        .iter().rev().take_while(|&x| *x == 0).count();

    let num_dist_codes = 30 - &h.code_sizes[1][1..30]
        .iter().rev().take_while(|&x| *x == 0).count();

    let mut code_sizes_to_pack = [0u8; TDEFL_MAX_HUFF_SYMBOLS_0 + TDEFL_MAX_HUFF_SYMBOLS_1];
    let mut packed_code_sizes = [0u8; TDEFL_MAX_HUFF_SYMBOLS_0 + TDEFL_MAX_HUFF_SYMBOLS_1];

    let total_code_sizes_to_pack = num_lit_codes + num_dist_codes;

    &code_sizes_to_pack[..num_lit_codes]
        .copy_from_slice(&h.code_sizes[0][..num_lit_codes]);

    &code_sizes_to_pack[num_lit_codes..total_code_sizes_to_pack]
        .copy_from_slice(&h.code_sizes[1][..num_dist_codes]);

    struct RLE {
        pub rle_z_count: u32,
        pub rle_repeat_count: u32,
        pub prev_code_size: u8,
    }

    let mut rle = RLE {
        rle_z_count: 0,
        rle_repeat_count: 0,
        prev_code_size: 0xFF,
    };

    let tdefl_rle_prev_code_size = |
        rle: &mut RLE,
        packed_code_sizes: &mut Cursor<&mut [u8]>,
        h: &mut HuffmanOxide,
    | -> io::Result<()> {
            if rle.rle_repeat_count != 0 {
                if rle.rle_repeat_count < 3 {
                    h.count[2][rle.prev_code_size as usize] = h.count[2][rle.prev_code_size as usize].wrapping_add(rle.rle_repeat_count as u16);
                    while rle.rle_repeat_count != 0 {
                        rle.rle_repeat_count -= 1;
                        packed_code_sizes.write(&[rle.prev_code_size][..])?;
                    }
                } else {
                    h.count[2][16] = h.count[2][16].wrapping_add(1);
                    packed_code_sizes.write(&[16, (rle.rle_repeat_count - 3) as u8][..])?;
                }
                rle.rle_repeat_count = 0;
            }

            Ok(())
        };

    let tdefl_rle_zero_code_size = |
        rle: &mut RLE,
        packed_code_sizes: &mut Cursor<&mut [u8]>,
        h: &mut HuffmanOxide,
    | -> io::Result<()> {
            if rle.rle_z_count != 0 {
                if rle.rle_z_count < 3 {
                    h.count[2][0] = h.count[2][0].wrapping_add(rle.rle_z_count as u16);
                    while rle.rle_z_count != 0 {
                        rle.rle_z_count -= 1;
                        packed_code_sizes.write(&[0][..])?;
                    }
                } else if rle.rle_z_count <= 10 {
                    h.count[2][17] = h.count[2][17].wrapping_add(1);
                    packed_code_sizes.write(&[17, (rle.rle_z_count - 3) as u8][..])?;
                } else {
                    h.count[2][18] = h.count[2][18].wrapping_add(1);
                    packed_code_sizes.write(&[18, (rle.rle_z_count - 11) as u8][..])?;
                }
                rle.rle_z_count = 0;
            }

            Ok(())
        };

    memset(&mut h.count[2][..TDEFL_MAX_HUFF_SYMBOLS_2], 0);

    let mut packed_code_sizes_cursor = Cursor::new(&mut packed_code_sizes[..]);
    for &code_size in &code_sizes_to_pack[..total_code_sizes_to_pack] {
        if code_size == 0 {
            tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
            rle.rle_z_count += 1;
            if rle.rle_z_count == 138 {
                tdefl_rle_zero_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
            }
        } else {
            tdefl_rle_zero_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
            if code_size != rle.prev_code_size {
                tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
                h.count[2][code_size as usize] = h.count[2][code_size as usize].wrapping_add(1);
                packed_code_sizes_cursor.write(&[code_size][..])?;
            } else {
                rle.rle_repeat_count += 1;
                if rle.rle_repeat_count == 6 {
                    tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
                }
            }
        }
        rle.prev_code_size = code_size;
    }

    if rle.rle_repeat_count != 0 {
        tdefl_rle_prev_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
    } else {
        tdefl_rle_zero_code_size(&mut rle, &mut packed_code_sizes_cursor, h)?;
    }

    tdefl_optimize_huffman_table_oxide(h, 2, TDEFL_MAX_HUFF_SYMBOLS_2, 7, false);

    output.put_bits(2, 2)?;

    output.put_bits((num_lit_codes - 257) as u32, 5)?;
    output.put_bits((num_dist_codes - 1) as u32, 5)?;

    let mut num_bit_lengths = 18 - TDEFL_PACKED_CODE_SIZE_SYMS_SWIZZLE
        .iter().rev().take_while(|&swizzle| h.code_sizes[2][*swizzle as usize] == 0).count();

    num_bit_lengths = cmp::max(4, num_bit_lengths + 1);
    output.put_bits(num_bit_lengths as u32 - 4, 4)?;
    for &swizzle in &TDEFL_PACKED_CODE_SIZE_SYMS_SWIZZLE[..num_bit_lengths] {
        output.put_bits(h.code_sizes[2][swizzle as usize] as u32, 3)?;
    }

    let mut packed_code_size_index = 0 as usize;
    let packed_code_sizes = packed_code_sizes_cursor.get_ref();
    while packed_code_size_index < packed_code_sizes_cursor.position() as usize {
        let code = packed_code_sizes[packed_code_size_index] as usize;
        packed_code_size_index += 1;
        assert!(code < TDEFL_MAX_HUFF_SYMBOLS_2);
        output.put_bits(h.codes[2][code] as u32, h.code_sizes[2][code] as u32)?;
        if code >= 16 {
            output.put_bits(packed_code_sizes[packed_code_size_index] as u32,
                            [2, 3, 7][code - 16])?;
            packed_code_size_index += 1;
        }
    }

    Ok(())
}

pub fn tdefl_start_static_block_oxide(
    h: &mut HuffmanOxide,
    output: &mut OutputBufferOxide
) -> io::Result<()> {
    memset(&mut h.code_sizes[0][0..144], 8);
    memset(&mut h.code_sizes[0][144..256], 9);
    memset(&mut h.code_sizes[0][256..280], 7);
    memset(&mut h.code_sizes[0][280..288], 8);

    memset(&mut h.code_sizes[1][..32], 5);

    tdefl_optimize_huffman_table_oxide(h, 0, 288, 15, true);
    tdefl_optimize_huffman_table_oxide(h, 1, 32, 15, true);

    output.put_bits(1, 2)
}

pub fn tdefl_compress_lz_codes_oxide(
    h: &mut HuffmanOxide,
    output: &mut OutputBufferOxide,
    lz_code_buf: &[u8]
) -> io::Result<bool> {
    let mut flags = 1;
    let mut bb = BitBuffer {
        bit_buffer: output.bit_buffer as u64,
        bits_in: output.bits_in
    };

    let mut i = 0;
    while i < lz_code_buf.len() {
        if flags == 1 {
            flags = lz_code_buf[i] as u32 | 0x100;
            i += 1;
        }

        if flags & 1 == 1 {
            flags >>= 1;

            let sym;
            let num_extra_bits;

            let match_len = lz_code_buf[i] as usize;
            let match_dist = read_unaligned_dict::<u16>(lz_code_buf, i as isize + 1);
            i += 3;

            assert!(h.code_sizes[0][TDEFL_LEN_SYM[match_len] as usize] != 0);
            bb.put_fast(h.codes[0][TDEFL_LEN_SYM[match_len] as usize] as u64,
                        h.code_sizes[0][TDEFL_LEN_SYM[match_len] as usize] as u32);
            bb.put_fast(match_len as u64 & MZ_BITMASKS[TDEFL_LEN_EXTRA[match_len] as usize] as u64,
                        TDEFL_LEN_EXTRA[match_len] as u32);

            if match_dist < 512 {
                sym = TDEFL_SMALL_DIST_SYM[match_dist as usize] as usize;
                num_extra_bits = TDEFL_SMALL_DIST_EXTRA[match_dist as usize] as usize;
            } else {
                sym = TDEFL_LARGE_DIST_SYM[(match_dist >> 8) as usize] as usize;
                num_extra_bits = TDEFL_LARGE_DIST_EXTRA[(match_dist >> 8) as usize] as usize;
            }

            assert!(h.code_sizes[1][sym] != 0);
            bb.put_fast(h.codes[1][sym] as u64, h.code_sizes[1][sym] as u32);
            bb.put_fast(match_dist as u64 & MZ_BITMASKS[num_extra_bits as usize] as u64, num_extra_bits as u32);
        } else {
            for _ in 0..3 {
                flags >>= 1;
                let lit = lz_code_buf[i];
                i += 1;

                assert!(h.code_sizes[0][lit as usize] != 0);
                bb.put_fast(h.codes[0][lit as usize] as u64, h.code_sizes[0][lit as usize] as u32);

                if flags & 1 == 1 || i >= lz_code_buf.len() {
                    break;
                }
            }
        }

        bb.flush(output)?;
    }

    output.bits_in = 0;
    output.bit_buffer = 0;
    while bb.bits_in != 0 {
        let n = cmp::min(bb.bits_in, 16);
        output.put_bits(bb.bit_buffer as u32 & MZ_BITMASKS[n as usize], n)?;
        bb.bit_buffer >>= n;
        bb.bits_in -= n;
    }

    output.put_bits(h.codes[0][256] as u32, h.code_sizes[0][256] as u32)?;

    Ok(true)
}

pub fn tdefl_compress_block_oxide(
    h: &mut HuffmanOxide,
    output: &mut OutputBufferOxide,
    lz: &LZOxide,
    static_block: bool
) -> io::Result<bool> {
    if static_block {
        tdefl_start_static_block_oxide(h, output)?;
    } else {
        tdefl_start_dynamic_block_oxide(h, output)?;
    }

    tdefl_compress_lz_codes_oxide(h, output, &lz.codes[..lz.code_position])
}

pub fn tdefl_flush_block_oxide(
    huff: &mut HuffmanOxide,
    lz: &mut LZOxide,
    dict: &mut DictOxide,
    params: &mut ParamsOxide,
    callback: &mut CallbackOxide,
    local_buf: &mut [u8],
    flush: TDEFLFlush
) -> io::Result<c_int> {
    let saved_bits;
    {
        let mut output = callback.out.new_output_buffer(local_buf, params.out_buf_ofs);
        output.bit_buffer = params.saved_bit_buffer;
        output.bits_in = params.saved_bits_in;

        let use_raw_block = (params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0) &&
            (dict.lookahead_pos - dict.code_buf_dict_pos) <= dict.size;

        assert!(params.flush_remaining == 0);
        params.flush_ofs = 0;
        params.flush_remaining = 0;

        lz.init_flag();

        if params.flags & TDEFL_WRITE_ZLIB_HEADER != 0 && params.block_index == 0 {
            output.put_bits(0x78, 8)?;
            output.put_bits(0x01, 8)?;
        }

        output.put_bits((flush == TDEFLFlush::Finish) as u32, 1)?;

        let saved_buffer = output.save();

        let mut comp_success = false;
        if !use_raw_block {
            let use_static = (params.flags & TDEFL_FORCE_ALL_STATIC_BLOCKS != 0) || (lz.total_bytes < 48);
            comp_success = tdefl_compress_block_oxide(huff, &mut output, lz, use_static)?;
        }

        let expanded = (lz.total_bytes != 0) &&
            (output.inner.position() - saved_buffer.pos + 1 >= lz.total_bytes as u64) &&
            (dict.lookahead_pos - dict.code_buf_dict_pos <= dict.size);

        if use_raw_block || expanded {
            output.load(saved_buffer);

            output.put_bits(0, 2)?;
            output.pad_to_bytes()?;

            for _ in 0..2 {
                output.put_bits(lz.total_bytes & 0xFFFF, 16)?;
                lz.total_bytes ^= 0xFFFF;
            }

            for i in 0..lz.total_bytes {
                let pos = (dict.code_buf_dict_pos + i) & TDEFL_LZ_DICT_SIZE_MASK;
                output.put_bits(dict.dict[pos as usize] as u32, 8)?;
            }
        } else if !comp_success {
            output.load(saved_buffer);
            tdefl_compress_block_oxide(huff, &mut output, lz, true)?;
        }

        if flush != TDEFLFlush::None {
            if flush == TDEFLFlush::Finish {
                output.pad_to_bytes()?;
                if params.flags & TDEFL_WRITE_ZLIB_HEADER != 0 {
                    let mut adler = params.adler32;
                    for _ in 0..4 {
                        output.put_bits((adler >> 24) & 0xFF, 8)?;
                        adler <<= 8;
                    }
                }
            } else {
                output.put_bits(0, 3)?;
                output.pad_to_bytes()?;
                output.put_bits(0, 16)?;
                output.put_bits(0xFFFF, 16)?;
            }
        }

        memset(&mut huff.count[0][..TDEFL_MAX_HUFF_SYMBOLS_0], 0);
        memset(&mut huff.count[1][..TDEFL_MAX_HUFF_SYMBOLS_1], 0);

        lz.code_position = 1;
        lz.flag_position = 0;
        lz.num_flags_left = 8;
        dict.code_buf_dict_pos += lz.total_bytes;
        lz.total_bytes = 0;
        params.block_index += 1;

        saved_bits = output.save();
    }

    let mut pos = saved_bits.pos;
    let local = saved_bits.local;
    params.saved_bit_buffer = saved_bits.bit_buffer;
    params.saved_bits_in = saved_bits.bits_in;

    if pos != 0 {
        match callback.out {
            CallbackOut::Func(ref mut cf) => {
                // TODO: callback about buf_in_size before put_buf_func
                let call_success = unsafe {
                    (cf.put_buf_func)(
                        &local_buf[0] as *const u8 as *const c_void,
                        pos as c_int,
                        cf.put_buf_user
                    )
                };

                if !call_success {
                    params.prev_return_status = TDEFLStatus::PutBufFailed;
                    return Ok(params.prev_return_status as c_int);
                }
            },
            CallbackOut::Buf(ref mut cb) => {
                if local {
                    let n = cmp::min(pos as usize, cb.out_buf.len() - params.out_buf_ofs);
                    (&mut cb.out_buf[params.out_buf_ofs..params.out_buf_ofs + n]).copy_from_slice(
                        &local_buf[..n]
                    );

                    params.out_buf_ofs += n;
                    pos -= n as u64;
                    if pos != 0 {
                        params.flush_ofs = n as c_uint;
                        params.flush_remaining = pos as c_uint;
                    }
                } else {
                    params.out_buf_ofs += pos as usize;
                }
            },
        }
    }

    Ok(params.flush_remaining as c_int)
}

fn read_unaligned_dict<T>(dict: &[u8], pos: isize) -> T {
    unsafe {
        ptr::read_unaligned((dict as *const [u8] as *const u8).offset(pos) as *const T)
    }
}

pub fn tdefl_find_match_oxide(
    dict: &DictOxide,
    lookahead_pos: c_uint,
    max_dist: c_uint,
    max_match_len: c_uint,
    mut match_dist: c_uint,
    mut match_len: c_uint
) -> (c_uint, c_uint) {
    assert!(max_match_len as usize <= TDEFL_MAX_MATCH_LEN);

    let pos = lookahead_pos & TDEFL_LZ_DICT_SIZE_MASK;
    let mut probe_pos = pos;
    let mut num_probes_left = dict.max_probes[(match_len >= 32) as usize];

    let mut c01: u16 = read_unaligned_dict(&dict.dict[..], (pos + match_len - 1) as isize);
    let s01: u16 = read_unaligned_dict(&dict.dict[..], pos as isize);

    if max_match_len <= match_len { return (match_dist, match_len) }

    loop {
        let mut dist = 0;
        'found: loop {
            num_probes_left -= 1;
            if num_probes_left == 0 { return (match_dist, match_len) }

            pub enum ProbeResult {
                OutOfBounds,
                Found,
                NotFound
            }

            let mut tdefl_probe = || -> ProbeResult {
                let next_probe_pos = dict.next[probe_pos as usize] as c_uint;

                dist = ((lookahead_pos - next_probe_pos) & 0xFFFF) as c_uint;
                if next_probe_pos == 0 || dist > max_dist {
                    return ProbeResult::OutOfBounds
                }

                probe_pos = next_probe_pos & TDEFL_LZ_DICT_SIZE_MASK;
                if read_unaligned_dict::<u16>(&dict.dict[..], (probe_pos + match_len - 1) as isize) == c01 {
                    ProbeResult::Found
                } else {
                    ProbeResult::NotFound
                }
            };

            for _ in 0..3 {
                match tdefl_probe() {
                    ProbeResult::OutOfBounds => return (match_dist, match_len),
                    ProbeResult::Found => break 'found,
                    ProbeResult::NotFound => ()
                }
            }
        }

        if dist == 0 { return (match_dist, match_len) }
        if read_unaligned_dict::<u16>(&dict.dict[..], probe_pos as isize) != s01 { continue }

        let mut probe_len = 32;
        let mut p = pos as isize;
        let mut q = probe_pos as isize;
        'probe: loop {
            for _ in 0..4 {
                p += 2;
                q += 2;
                if read_unaligned_dict::<u16>(&dict.dict[..], p) != read_unaligned_dict(&dict.dict[..], q) {
                    break 'probe;
                }
            }
            probe_len -= 1;
            if probe_len == 0 {
                return (dist, cmp::min(max_match_len, TDEFL_MAX_MATCH_LEN as c_uint))
            }
        }

        probe_len = (p - pos as isize + (dict.dict[p as usize] == dict.dict[q as usize]) as isize) as c_uint;
        if probe_len > match_len {
            match_dist = dist;
            match_len = cmp::min(max_match_len, probe_len);
            if match_len == max_match_len {
                return (match_dist, match_len);
            }
            c01 = read_unaligned_dict(&dict.dict[..], (pos + match_len - 1) as isize);
        }
    }
}

pub fn tdefl_record_literal_oxide(h: &mut HuffmanOxide, lz: &mut LZOxide, lit: u8) {
    lz.total_bytes += 1;
    lz.write_code(lit);

    *lz.get_flag() >>= 1;
    lz.consume_flag();

    h.count[0][lit as usize] += 1;
}

pub fn tdefl_record_match_oxide(
    h: &mut HuffmanOxide,
    lz: &mut LZOxide,
    mut match_len: c_uint,
    mut match_dist: c_uint
) {
    assert!(match_len >= TDEFL_MIN_MATCH_LEN);
    assert!(match_dist >= 1);
    assert!(match_dist as usize <= TDEFL_LZ_DICT_SIZE);

    lz.total_bytes += match_len;
    match_dist -= 1;
    match_len -= TDEFL_MIN_MATCH_LEN as u32;
    lz.write_code(match_len as u8);
    lz.write_code(match_dist as u8);
    lz.write_code((match_dist >> 8) as u8);

    *lz.get_flag() >>= 1;
    *lz.get_flag() |= 0x80;
    lz.consume_flag();

    let symbol = if match_dist < 512 {
        TDEFL_SMALL_DIST_SYM[match_dist as usize]
    } else {
        TDEFL_LARGE_DIST_SYM[((match_dist >> 8) & 127) as usize]
    } as usize;
    h.count[1][symbol] += 1;
    h.count[0][TDEFL_LEN_SYM[match_len as usize] as usize] += 1;
}

pub fn tdefl_compress_normal_oxide(
    huff: &mut HuffmanOxide,
    lz: &mut LZOxide,
    dict: &mut DictOxide,
    params: &mut ParamsOxide,
    callback: &mut CallbackOxide,
    local_buf: &mut [u8]
) -> bool {
    let mut src_pos = params.src_pos;
    let mut src_buf_left = params.src_buf_left;
    while src_buf_left != 0 || (params.flush != TDEFLFlush::None && dict.lookahead_size != 0) {
        let in_buf = callback.in_buf.expect("Unexpected null in_buf"); // TODO: make connection  params.src_buf_left <-> in_buf
        let num_bytes_to_process = cmp::min(src_buf_left, TDEFL_MAX_MATCH_LEN - dict.lookahead_size as usize);
        src_buf_left -= num_bytes_to_process;
        if dict.lookahead_size + dict.size >= TDEFL_MIN_MATCH_LEN - 1 {
            let mut dst_pos = (dict.lookahead_pos + dict.lookahead_size) & TDEFL_LZ_DICT_SIZE_MASK;
            let mut ins_pos = dict.lookahead_pos + dict.lookahead_size - 2;
            let mut hash = ((dict.dict[(ins_pos & TDEFL_LZ_DICT_SIZE_MASK) as usize] as c_uint) << TDEFL_LZ_HASH_SHIFT) ^
                (dict.dict[((ins_pos + 1) & TDEFL_LZ_DICT_SIZE_MASK) as usize] as c_uint);

            dict.lookahead_size += num_bytes_to_process as c_uint;
            for &c in &in_buf[src_pos..src_pos + num_bytes_to_process] {
                dict.dict[dst_pos as usize] = c;
                if (dst_pos as usize) < TDEFL_MAX_MATCH_LEN - 1 {
                    dict.dict[TDEFL_LZ_DICT_SIZE + dst_pos as usize] = c;
                }

                hash = ((hash << TDEFL_LZ_HASH_SHIFT) ^ (c as c_uint)) & (TDEFL_LZ_HASH_SIZE as c_uint - 1);
                dict.next[(ins_pos & TDEFL_LZ_DICT_SIZE_MASK) as usize] = dict.hash[hash as usize];
                dict.hash[hash as usize] = ins_pos as u16;
                dst_pos = (dst_pos + 1) & TDEFL_LZ_DICT_SIZE_MASK;
                ins_pos += 1;
            }
            src_pos += num_bytes_to_process;
        } else {
            for &c in &in_buf[src_pos..src_pos + num_bytes_to_process] {
                let dst_pos = (dict.lookahead_pos + dict.lookahead_size) & TDEFL_LZ_DICT_SIZE_MASK;
                dict.dict[dst_pos as usize] = c;
                if (dst_pos as usize) < TDEFL_MAX_MATCH_LEN - 1 {
                    dict.dict[TDEFL_LZ_DICT_SIZE + dst_pos as usize] = c;
                }

                dict.lookahead_size += 1;
                if dict.lookahead_size + dict.size >= TDEFL_MIN_MATCH_LEN {
                    let ins_pos = dict.lookahead_pos + dict.lookahead_size - 3;
                    let hash = (((dict.dict[(ins_pos & TDEFL_LZ_DICT_SIZE_MASK) as usize] as c_uint) << (TDEFL_LZ_HASH_SHIFT * 2)) ^
                        (((dict.dict[((ins_pos + 1) & TDEFL_LZ_DICT_SIZE_MASK) as usize] as c_uint) << TDEFL_LZ_HASH_SHIFT) ^ (c as c_uint))) &
                        (TDEFL_LZ_HASH_SIZE as c_uint - 1);

                    dict.next[(ins_pos & TDEFL_LZ_DICT_SIZE_MASK) as usize] = dict.hash[hash as usize];
                    dict.hash[hash as usize] = ins_pos as u16;
                }
            }

            src_pos += num_bytes_to_process;
        }

        dict.size = cmp::min(TDEFL_LZ_DICT_SIZE as c_uint - dict.lookahead_size, dict.size);
        if params.flush == TDEFLFlush::None && (dict.lookahead_size as usize) < TDEFL_MAX_MATCH_LEN { break }

        let mut len_to_move = 1;
        let mut cur_match_dist = 0;
        let mut cur_match_len = if params.saved_match_len != 0 { params.saved_match_len } else { TDEFL_MIN_MATCH_LEN - 1 };
        let cur_pos = dict.lookahead_pos & TDEFL_LZ_DICT_SIZE_MASK;
        if params.flags & (TDEFL_RLE_MATCHES | TDEFL_FORCE_ALL_RAW_BLOCKS) != 0 {
            if dict.size != 0 && params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS == 0 {
                let c = dict.dict[((cur_pos.wrapping_sub(1)) & TDEFL_LZ_DICT_SIZE_MASK) as usize];
                cur_match_len = dict.dict[cur_pos as usize..(cur_pos + dict.lookahead_size) as usize]
                    .iter().take_while(|&x| *x == c).count() as c_uint;
                if cur_match_len < TDEFL_MIN_MATCH_LEN { cur_match_len = 0 } else { cur_match_dist = 1 }
            }
        } else {
            let dist_len = tdefl_find_match_oxide(
                dict,
                dict.lookahead_pos,
                dict.size,
                dict.lookahead_size,
                cur_match_dist,
                cur_match_len
            );
            cur_match_dist = dist_len.0;
            cur_match_len = dist_len.1;
        }

        let far_and_small = cur_match_len == TDEFL_MIN_MATCH_LEN && cur_match_dist >= 8 * 1024;
        let filter_small = params.flags & TDEFL_FILTER_MATCHES != 0 && cur_match_len <= 5;
        if far_and_small || filter_small || cur_pos == cur_match_dist {
            cur_match_dist = 0;
            cur_match_len = 0;
        }

        if params.saved_match_len != 0 {
            if cur_match_len > params.saved_match_len {
                tdefl_record_literal_oxide(huff, lz, params.saved_lit);
                if cur_match_len >= 128 {
                    tdefl_record_match_oxide(huff, lz, cur_match_len, cur_match_dist);
                    params.saved_match_len = 0;
                    len_to_move = cur_match_len;
                } else {
                    params.saved_lit = dict.dict[cur_pos as usize];
                    params.saved_match_dist = cur_match_dist;
                    params.saved_match_len = cur_match_len;
                }
            } else {
                tdefl_record_match_oxide(huff, lz, params.saved_match_len, params.saved_match_dist);
                len_to_move = params.saved_match_len - 1;
                params.saved_match_len = 0;
            }
        } else if cur_match_dist == 0 {
            tdefl_record_literal_oxide(huff, lz, dict.dict[cmp::min(cur_pos as usize, dict.dict.len() - 1)]);
        } else if params.greedy_parsing || (params.flags & TDEFL_RLE_MATCHES != 0) || cur_match_len >= 128 {
            tdefl_record_match_oxide(huff, lz, cur_match_len, cur_match_dist);
            len_to_move = cur_match_len;
        } else {
            params.saved_lit = dict.dict[cmp::min(cur_pos as usize, dict.dict.len() - 1)];
            params.saved_match_dist = cur_match_dist;
            params.saved_match_len = cur_match_len;
        }

        dict.lookahead_pos += len_to_move;
        assert!(dict.lookahead_size >= len_to_move);
        dict.lookahead_size -= len_to_move;
        dict.size = cmp::min(dict.size + len_to_move, TDEFL_LZ_DICT_SIZE as c_uint);

        let lz_buf_tight = lz.code_position > TDEFL_LZ_CODE_BUF_SIZE - 8;
        let raw = params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0;
        let fat = ((lz.code_position * 115) >> 7) >= lz.total_bytes as usize;
        let fat_or_raw = (lz.total_bytes > 31 * 1024) && (fat || raw);

        if lz_buf_tight || fat_or_raw {
            params.src_pos = src_pos;
            params.src_buf_left = src_buf_left;

            let n = tdefl_flush_block_oxide(
                huff,
                lz,
                dict,
                params,
                callback,
                local_buf,
                TDEFLFlush::None,
            ).unwrap_or(TDEFLStatus::PutBufFailed as c_int);
            if n != 0 { return n > 0 }
        }
    }

    params.src_pos = src_pos;
    params.src_buf_left = src_buf_left;
    true
}

const TDEFL_COMP_FAST_LOOKAHEAD_SIZE: c_uint = 4096;

pub fn tdefl_compress_fast_oxide(
    huff: &mut HuffmanOxide,
    lz: &mut LZOxide,
    dict: &mut DictOxide,
    params: &mut ParamsOxide,
    callback: &mut CallbackOxide,
    local_buf: &mut [u8]
) -> bool {
    let mut cur_pos = dict.lookahead_pos & TDEFL_LZ_DICT_SIZE_MASK;
    let in_buf = callback.in_buf.expect("Unexpected null in_buf"); // TODO: make connection  params.src_buf_left <-> in_buf
    while params.src_buf_left > 0 || (params.flush != TDEFLFlush::None && dict.lookahead_size > 0) {
        let mut dst_pos = ((dict.lookahead_pos + dict.lookahead_size) & TDEFL_LZ_DICT_SIZE_MASK) as usize;
        let mut num_bytes_to_process = cmp::min(params.src_buf_left, (TDEFL_COMP_FAST_LOOKAHEAD_SIZE - dict.lookahead_size) as usize);
        params.src_buf_left -= num_bytes_to_process;
        dict.lookahead_size += num_bytes_to_process as c_uint;

        while num_bytes_to_process != 0 {
            let n = cmp::min(TDEFL_LZ_DICT_SIZE - dst_pos , num_bytes_to_process);
            &mut dict.dict[dst_pos..dst_pos + n]
                .copy_from_slice(&in_buf[params.src_pos..params.src_pos + n]);

            if dst_pos < TDEFL_MAX_MATCH_LEN - 1 {
                let m = cmp::min(n, TDEFL_MAX_MATCH_LEN - 1 - dst_pos);
                &mut dict.dict[dst_pos + TDEFL_LZ_DICT_SIZE..dst_pos + TDEFL_LZ_DICT_SIZE + m]
                    .copy_from_slice(&in_buf[params.src_pos..params.src_pos + m]);
            }

            params.src_pos += n;
            dst_pos = (dst_pos + n) & TDEFL_LZ_DICT_SIZE_MASK as usize;
            num_bytes_to_process -= n;
        }

        dict.size = cmp::min(TDEFL_LZ_DICT_SIZE as c_uint - dict.lookahead_size, dict.size);
        if params.flush == TDEFLFlush::None && dict.lookahead_size < TDEFL_COMP_FAST_LOOKAHEAD_SIZE {
            break;
        }

        while dict.lookahead_size >= 4 {
            let mut cur_match_len = 1;
            let first_trigram = read_unaligned_dict::<u32>(&dict.dict[..], cur_pos as isize) & 0xFFFFFF;
            let hash = (first_trigram ^ (first_trigram >> (24 - (TDEFL_LZ_HASH_BITS - 8)))) & TDEFL_LEVEL1_HASH_SIZE_MASK;
            let mut probe_pos = dict.hash[hash as usize] as u32;
            dict.hash[hash as usize] = dict.lookahead_pos as u16;

            let mut cur_match_dist = (dict.lookahead_pos - probe_pos) as u16;
            if cur_match_dist as u32 <= dict.size {
                probe_pos &= TDEFL_LZ_DICT_SIZE_MASK;
                let trigram = read_unaligned_dict::<u32>(&dict.dict[..], probe_pos as isize) & 0xFFFFFF;
                if first_trigram == trigram {
                    let mut p = cur_pos as isize;
                    let mut q = probe_pos as isize;
                    let mut probe_len = 32;

                    'probe: loop {
                        for _ in 0..4 {
                            p += 2;
                            q += 2;
                            if read_unaligned_dict::<u16>(&dict.dict[..], p) != read_unaligned_dict(&dict.dict[..], q) {
                                cur_match_len = (p as u32 - cur_pos) + (dict.dict[p as usize] == dict.dict[q as usize]) as u32;
                                break 'probe;
                            }
                        }
                        probe_len -= 1;
                        if probe_len == 0 {
                            cur_match_len = if cur_match_dist == 0 {
                                0
                            } else {
                                TDEFL_MAX_MATCH_LEN as u32
                            };
                            break 'probe;
                        }
                    }

                    if cur_match_len < TDEFL_MIN_MATCH_LEN || (cur_match_len == TDEFL_MIN_MATCH_LEN && cur_match_dist >= 8 * 1024) {
                        cur_match_len = 1;
                        lz.write_code(first_trigram as u8);
                        *lz.get_flag() >>= 1;
                        huff.count[0][first_trigram as u8 as usize] += 1;
                    } else {
                        cur_match_len = cmp::min(cur_match_len, dict.lookahead_size);
                        assert!(cur_match_len >= TDEFL_MIN_MATCH_LEN);
                        assert!(cur_match_dist >= 1);
                        assert!(cur_match_dist as usize <= TDEFL_LZ_DICT_SIZE);
                        cur_match_dist -= 1;

                        lz.write_code((cur_match_len - TDEFL_MIN_MATCH_LEN) as u8);
                        unsafe {
                            ptr::write_unaligned(
                                (&mut lz.codes[0] as *mut u8).offset(lz.code_position as isize) as *mut u16,
                                cur_match_dist as u16
                            );
                            lz.code_position += 2;
                        }

                        *lz.get_flag() >>= 1;
                        *lz.get_flag() |= 0x80;
                        if cur_match_dist < 512 {
                            huff.count[1][TDEFL_SMALL_DIST_SYM[cur_match_dist as usize] as usize] += 1;
                        } else {
                            huff.count[1][TDEFL_LARGE_DIST_SYM[(cur_match_dist >> 8) as usize] as usize] += 1;
                        }

                        huff.count[0][TDEFL_LEN_SYM[(cur_match_len - TDEFL_MIN_MATCH_LEN) as usize] as usize] += 1;
                    }
                } else {
                    lz.write_code(first_trigram as u8);
                    *lz.get_flag() >>= 1;
                    huff.count[0][first_trigram as u8 as usize] += 1;
                }

                lz.consume_flag();
                lz.total_bytes += cur_match_len;
                dict.lookahead_pos += cur_match_len;
                dict.size = cmp::min(dict.size + cur_match_len, TDEFL_LZ_DICT_SIZE as u32);
                cur_pos = (cur_pos + cur_match_len) & TDEFL_LZ_DICT_SIZE_MASK;
                assert!(dict.lookahead_size >= cur_match_len);
                dict.lookahead_size -= cur_match_len;

                if lz.code_position > TDEFL_LZ_CODE_BUF_SIZE - 8 {
                    let n = match tdefl_flush_block_oxide(
                        huff,
                        lz,
                        dict,
                        params,
                        callback,
                        local_buf,
                        TDEFLFlush::None
                    ) {
                        Err(_) => {params.prev_return_status = TDEFLStatus::PutBufFailed; -1},
                        Ok(status) => status
                    };
                    if n != 0 { return n > 0 }
                }
            }
        }

        while dict.lookahead_size != 0 {
            let lit = dict.dict[cur_pos as usize];
            lz.total_bytes += 1;
            lz.write_code(lit);
            *lz.get_flag() >>= 1;
            lz.consume_flag();

            huff.count[0][lit as usize] += 1;
            dict.lookahead_pos += 1;
            dict.size = cmp::min(dict.size + 1, TDEFL_LZ_DICT_SIZE as u32);
            cur_pos = (cur_pos + 1) & TDEFL_LZ_DICT_SIZE_MASK;
            dict.lookahead_size -= 1;

            if lz.code_position > TDEFL_LZ_CODE_BUF_SIZE - 8 {
                let n = match tdefl_flush_block_oxide(
                    huff,
                    lz,
                    dict,
                    params,
                    callback,
                    local_buf,
                    TDEFLFlush::None
                ) {
                    Err(_) => {params.prev_return_status = TDEFLStatus::PutBufFailed; -1},
                    Ok(status) => status
                };
                if n != 0 { return n > 0 }
            }
        }
    }

    true
}

pub fn tdefl_flush_output_buffer_oxide(
    c: &mut CallbackOxide,
    p: &mut ParamsOxide,
    local_buf: &[u8]
) -> (TDEFLStatus, usize, usize) {
    let mut res = (TDEFLStatus::Okay, p.src_pos, 0);
    if let CallbackOut::Buf(ref mut cb) = c.out {
        let n = cmp::min(cb.out_buf.len() - p.out_buf_ofs, p.flush_remaining as usize);
        if n != 0 {
            (&mut cb.out_buf[p.out_buf_ofs..p.out_buf_ofs + n]).copy_from_slice(
                &local_buf[p.flush_ofs as usize.. p.flush_ofs as usize + n]
            );
        }
        p.flush_ofs += n as c_uint;
        p.flush_remaining -= n as c_uint;
        p.out_buf_ofs += n;
        res.2 = p.out_buf_ofs;
    }

    if p.finished && p.flush_remaining == 0 {
        res.0 = TDEFLStatus::Done
    }
    res
}

pub fn tdefl_compress_oxide(
    d: &mut CompressorOxide,
    callback: &mut CallbackOxide,
    flush: TDEFLFlush
) -> (TDEFLStatus, usize, usize) {
    d.params.src_buf_left = callback.in_buf.map_or(0, |buf| buf.len());
    d.params.out_buf_ofs = 0;
    d.params.src_pos = 0;

    let prev_ok = d.params.prev_return_status == TDEFLStatus::Okay;
    let flush_finish_once = d.params.flush != TDEFLFlush::Finish ||
        flush == TDEFLFlush::Finish;

    d.params.flush = flush;
    if !prev_ok || !flush_finish_once {
        d.params.prev_return_status = TDEFLStatus::BadParam;
        return (d.params.prev_return_status, 0, 0);
    }

    if d.params.flush_remaining != 0 || d.params.finished {
        let res = tdefl_flush_output_buffer_oxide(
            callback,
            &mut d.params,
            &d.local_buf[..]
        );
        d.params.prev_return_status = res.0;
        return res;
    }

    let one_probe = d.params.flags & TDEFL_MAX_PROBES_MASK as u32 == 1;
    let greedy = d.params.flags & TDEFL_GREEDY_PARSING_FLAG != 0;
    let filter_or_rle_or_raw = d.params.flags & (TDEFL_FILTER_MATCHES | TDEFL_FORCE_ALL_RAW_BLOCKS | TDEFL_RLE_MATCHES) != 0;

    let compress_success = if one_probe && greedy && !filter_or_rle_or_raw {
        tdefl_compress_fast_oxide(
            &mut d.huff,
            &mut d.lz,
            &mut d.dict,
            &mut d.params,
            callback,
            &mut d.local_buf[..]
        )
    } else {
        tdefl_compress_normal_oxide(
            &mut d.huff,
            &mut d.lz,
            &mut d.dict,
            &mut d.params,
            callback,
            &mut d.local_buf[..]
        )
    };

    if !compress_success {
        return (d.params.prev_return_status, d.params.src_pos, d.params.out_buf_ofs);
    }

    if let Some(in_buf) = callback.in_buf {
        if d.params.flags & (TDEFL_WRITE_ZLIB_HEADER | TDEFL_COMPUTE_ADLER32) != 0 {
            d.params.adler32 = ::mz_adler32_oxide(
                d.params.adler32,
                &in_buf[..d.params.src_pos]
            );
        }
    }

    let flush_none = d.params.flush == TDEFLFlush::None;
    let remaining = d.params.src_buf_left != 0 || d.params.flush_remaining != 0;
    if !flush_none && d.dict.lookahead_size == 0 && !remaining {
        let flush = d.params.flush;
        match tdefl_flush_block_oxide(
            &mut d.huff,
            &mut d.lz,
            &mut d.dict,
            &mut d.params,
            callback,
            &mut d.local_buf[..],
            flush
        ) {
            Err(_) => {
                d.params.prev_return_status = TDEFLStatus::PutBufFailed;
                return (d.params.prev_return_status, d.params.src_pos, d.params.out_buf_ofs);
            },
            Ok(x) if x < 0 => return (d.params.prev_return_status, d.params.src_pos, d.params.out_buf_ofs),
            _ => {
                d.params.finished = d.params.flush == TDEFLFlush::Finish;
                if d.params.flush == TDEFLFlush::Full {
                    memset(&mut d.dict.hash[..], 0);
                    memset(&mut d.dict.next[..], 0);
                    d.dict.size = 0;
                }
            },
        }
    }

    let res = tdefl_flush_output_buffer_oxide(
        callback,
        &mut d.params,
        &d.local_buf[..]
    );
    d.params.prev_return_status = res.0;

    res
}

pub fn tdefl_get_adler32_oxide(d: &CompressorOxide) -> c_uint {
    d.params.adler32
}

pub fn tdefl_get_prev_return_status_oxide(d: &CompressorOxide) -> TDEFLStatus {
    d.params.prev_return_status
}

pub fn tdefl_get_flags_oxide(d: &CompressorOxide) -> c_int {
    d.params.flags as c_int
}

pub fn tdefl_create_comp_flags_from_zip_params_oxide(
    level: c_int,
    window_bits: c_int,
    strategy: c_int
) -> c_uint {
    let num_probes = (if level >= 0 {
        cmp::min(10, level)
    } else {
        ::CompressionLevel::DefaultLevel as c_int
    }) as usize;
    let greedy = if level <= 3 { TDEFL_GREEDY_PARSING_FLAG } else { 0 } as c_uint;
    let mut comp_flags = TDEFL_NUM_PROBES[num_probes] | greedy;

    if window_bits > 0 {
        comp_flags |= TDEFL_WRITE_ZLIB_HEADER as c_uint;
    }

    if level == 0 {
        comp_flags |= TDEFL_FORCE_ALL_RAW_BLOCKS;
    } else if strategy == ::CompressionStrategy::Filtered as c_int {
        comp_flags |= TDEFL_FILTER_MATCHES;
    } else if strategy == ::CompressionStrategy::HuffmanOnly as c_int {
        comp_flags &= !TDEFL_MAX_PROBES_MASK as c_uint;
    } else if strategy == ::CompressionStrategy::Fixed as c_int {
        comp_flags |= TDEFL_FORCE_ALL_STATIC_BLOCKS;
    } else if strategy == ::CompressionStrategy::RLE as c_int {
        comp_flags |= TDEFL_RLE_MATCHES;
    }

    comp_flags
}
