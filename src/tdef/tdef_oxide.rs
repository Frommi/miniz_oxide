use super::*;

fn memset<T : Clone>(slice: &mut [T], val: T) {
    for x in slice { *x = val.clone() }
}

pub struct HuffmanOxide<'a> {
    pub count: &'a mut [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub codes: &'a mut [[u16; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES],
    pub code_sizes: &'a mut [[u8; TDEFL_MAX_HUFF_SYMBOLS]; TDEFL_MAX_HUFF_TABLES]
}

pub struct OutputBufferOxide<'a> {
    pub d: *mut tdefl_compressor,

    pub inner: Cursor<&'a mut [u8]>,

    pub bit_buffer: &'a mut u32,
    pub bits_in: &'a mut u32
}

pub struct DictOxide<'a> {
    pub d: *mut tdefl_compressor,

    pub max_probes: &'a mut [c_uint; 2],
    pub dict: &'a mut [u8; TDEFL_LZ_DICT_SIZE + TDEFL_MAX_MATCH_LEN - 1],
    pub next: &'a mut [u16; TDEFL_LZ_DICT_SIZE],
    pub hash: &'a mut [u16; TDEFL_LZ_DICT_SIZE],

    pub src_pos: usize,

    pub code_buf_dict_pos: c_uint,
    pub lookahead_size: c_uint,
    pub lookahead_pos: c_uint,
    pub size: c_uint
}

pub struct LZOxide<'a> {
    pub d: *mut tdefl_compressor,

    pub codes: &'a mut [u8; TDEFL_LZ_CODE_BUF_SIZE],
    pub code_position: usize,
    pub flag_position: usize,

    pub total_bytes: c_uint,
    pub num_flags_left: c_uint
}

pub struct ParamsOxide {
    pub d: *mut tdefl_compressor,

    pub flags: c_uint,
    pub greedy_parsing: bool,
    pub block_index: c_uint,

    pub saved_match_dist: c_uint,
    pub saved_match_len: libc::c_uint,
    pub saved_lit: u8,

    pub flush: TDEFLFlush,
    pub flush_ofs: c_uint,
    pub flush_remaining: c_uint,
    pub adler32: c_uint,

    pub src_buf_left: usize,

    pub out_buf_ofs: usize,
    pub prev_return_status: TDEFLStatus
}

pub struct CallbackOxide<'a> {
    pub put_buf_func: tdefl_put_buf_func_ptr,
    pub put_buf_user: Option<&'a mut c_void>,

    pub in_buf: Option<&'a [u8]>,
    pub out_buf: Option<&'a mut [u8]>,

    pub in_buf_size: Option<&'a mut usize>,
    pub out_buf_size: Option<&'a mut usize>
}

impl<'a> DictOxide<'a> {
    pub unsafe fn new(d: *mut tdefl_compressor) -> Self {
        let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");
        DictOxide {
            d: d,

            max_probes: &mut d.m_max_probes,
            dict: &mut d.m_dict,
            hash: &mut d.m_hash,
            next: &mut d.m_next,

            src_pos: d.m_pSrc as usize - d.m_pIn_buf as usize,

            code_buf_dict_pos: d.m_lz_code_buf_dict_pos,
            lookahead_size: d.m_lookahead_size,
            lookahead_pos: d.m_lookahead_pos,
            size: d.m_dict_size
        }
    }
}

impl<'a> Drop for DictOxide<'a> {
    fn drop(&mut self) {
        let mut d = unsafe {
            self.d.as_mut().expect("Bad tdefl_compressor pointer")
        };
        d.m_pSrc = unsafe {
            d.m_pIn_buf.offset(self.src_pos as isize) as *const u8
        };
        d.m_lz_code_buf_dict_pos = self.code_buf_dict_pos;
        d.m_lookahead_size = self.lookahead_size;
        d.m_lookahead_pos = self.lookahead_pos;
        d.m_dict_size = self.size;
    }
}

impl<'a> HuffmanOxide<'a> {
    pub unsafe fn new(d: *mut tdefl_compressor) -> Self {
        let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");
        HuffmanOxide {
            count: &mut d.m_huff_count,
            code_sizes: &mut d.m_huff_code_sizes,
            codes: &mut d.m_huff_codes
        }
    }
}

impl<'a> Drop for LZOxide<'a> {
    fn drop(&mut self) {
        let mut d = unsafe {
            self.d.as_mut().expect("Bad tdefl_compressor pointer")
        };

        d.m_pLZ_code_buf = &mut d.m_lz_code_buf[self.code_position];
        d.m_pLZ_flags = &mut d.m_lz_code_buf[self.flag_position];
        d.m_total_lz_bytes = self.total_bytes;
        d.m_num_flags_left = self.num_flags_left;
    }
}

impl<'a> LZOxide<'a> {
    pub unsafe fn new(d: *mut tdefl_compressor) -> Self {
        let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");
        let code_index = d.m_pLZ_code_buf as usize - &d.m_lz_code_buf[0] as *const u8 as usize;
        let flag_index = d.m_pLZ_flags as usize - &d.m_lz_code_buf[0] as *const u8 as usize;
        LZOxide {
            d: d,

            codes: &mut d.m_lz_code_buf,
            code_position: code_index,
            flag_position: flag_index,

            total_bytes: d.m_total_lz_bytes,
            num_flags_left: d.m_num_flags_left,
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

struct SavedOutputBufferOxide {
    pub pos: u64,
    pub bit_buffer: u32,
    pub bits_in: u32
}

impl<'a> Drop for OutputBufferOxide<'a> {
    fn drop(&mut self) {
        unsafe {
            let mut d = self.d.as_mut().expect("Bad tdefl_compressor pointer");
            d.m_pOutput_buf = d.m_pOutput_buf.offset(self.inner.position() as isize);
        }
    }
}

impl<'a> OutputBufferOxide<'a> {
    pub unsafe fn new(d: *mut tdefl_compressor) -> Self {
        let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");

        let len = d.m_pOutput_buf_end as usize - d.m_pOutput_buf as usize;
        let cursor = Cursor::new(
            slice::from_raw_parts_mut(d.m_pOutput_buf, len as usize)
        );

        OutputBufferOxide {
            d: d,
            inner: cursor,
            bit_buffer: &mut d.m_bit_buffer,
            bits_in: &mut d.m_bits_in
        }
    }

    pub unsafe fn choose_buffer_new(d: *mut tdefl_compressor) -> (Self, bool) {
        let mut d = d.as_mut().expect("Bad tdefl_compressor pointer");

        let choose_local;
        let chosen_buffer = if d.m_pPut_buf_func.is_none() && *d.m_pOut_buf_size - d.m_out_buf_ofs >= TDEFL_OUT_BUF_SIZE {
            choose_local = false;
            (d.m_pOut_buf as *mut u8).offset(d.m_out_buf_ofs as isize)
        } else {
            choose_local = true;
            &mut d.m_output_buf[0]
        };

        d.m_pOutput_buf = chosen_buffer;
        d.m_pOutput_buf_end = d.m_pOutput_buf.offset(TDEFL_OUT_BUF_SIZE as isize - 16);

        (OutputBufferOxide::new(d), choose_local)
    }

    fn put_bits(&mut self, bits: u32, len: u32) -> io::Result<()> {
        assert!(bits <= ((1u32 << len) - 1u32));
        *self.bit_buffer |= bits << *self.bits_in;
        *self.bits_in += len;
        while *self.bits_in >= 8 {
            self.inner.write(&[*self.bit_buffer as u8][..])?;
            *self.bit_buffer >>= 8;
            *self.bits_in -= 8;
        }
        Ok(())
    }

    fn save(&self) -> SavedOutputBufferOxide {
        SavedOutputBufferOxide {
            pos: self.inner.position(),
            bit_buffer: *self.bit_buffer,
            bits_in: *self.bits_in
        }
    }

    fn load(&mut self, saved: SavedOutputBufferOxide) {
        self.inner.set_position(saved.pos);
        *self.bit_buffer = saved.bit_buffer;
        *self.bits_in = saved.bits_in;
    }

    fn pad_to_bytes(&mut self) -> io::Result<()> {
        if *self.bits_in != 0 {
            let len = 8 - *self.bits_in;
            self.put_bits(0, len)?;
        }

        Ok(())
    }
}

impl Drop for ParamsOxide {
    fn drop(&mut self) {
        let mut d = unsafe {
            self.d.as_mut().expect("Bad tdefl_compressor pointer")
        };

        d.m_flags = self.flags;
        d.m_greedy_parsing = self.greedy_parsing as c_int;
        d.m_block_index = self.block_index;
        d.m_saved_match_dist = self.saved_match_dist;
        d.m_saved_match_len = self.saved_match_len;
        d.m_saved_lit = self.saved_lit as c_uint;
        d.m_flush = self.flush;
        d.m_output_flush_ofs = self.flush_ofs;
        d.m_output_flush_remaining = self.flush_remaining;
        d.m_adler32 = self.adler32;
        d.m_src_buf_left = self.src_buf_left;
        d.m_out_buf_ofs = self.out_buf_ofs;
        d.m_prev_return_status = self.prev_return_status;
    }
}

impl ParamsOxide {
    pub unsafe fn new(d: *mut tdefl_compressor) -> Self {
        let d = d.as_mut().expect("Bad tdefl_compressor pointer");

        ParamsOxide {
            d: d,
            flags: d.m_flags,
            greedy_parsing: d.m_greedy_parsing != 0,
            block_index: d.m_block_index,
            saved_match_dist: d.m_saved_match_dist,
            saved_match_len: d.m_saved_match_len,
            saved_lit: d.m_saved_lit as u8,
            flush: d.m_flush,
            flush_ofs: d.m_output_flush_ofs,
            flush_remaining: d.m_output_flush_remaining,
            adler32: d.m_adler32,
            src_buf_left: d.m_src_buf_left,
            out_buf_ofs: d.m_out_buf_ofs,
            prev_return_status: d.m_prev_return_status
        }
    }
}

impl<'a> CallbackOxide<'a> {
    pub unsafe fn new(d: *mut tdefl_compressor) -> Self {
        let d = d.as_mut().expect("Bad tdefl_compressor pointer");

        let mut in_size = d.m_pIn_buf_size.as_mut();
        let mut out_size = d.m_pOut_buf_size.as_mut();

        let in_buf = d.m_pIn_buf.as_ref().and_then(|buf| in_size.as_mut().map(|size| (buf, size)))
            .map(|(buf, size)| slice::from_raw_parts(buf as *const c_void as *const u8, **size));
        let out_buf = d.m_pOut_buf.as_mut().and_then(|buf| out_size.as_mut().map(|size| (buf, size)))
            .map(|(buf, size)| slice::from_raw_parts_mut(buf as *mut c_void as *mut u8, **size));

        CallbackOxide {
            put_buf_func: d.m_pPut_buf_func,
            put_buf_user: d.m_pPut_buf_user.as_mut(),

            in_buf_size: in_size,
            out_buf_size: out_size,

            in_buf: in_buf,
            out_buf: out_buf,
        }
    }
}

pub fn tdefl_radix_sort_syms_oxide<'a>(symbols0: &'a mut [tdefl_sym_freq],
                                       symbols1: &'a mut [tdefl_sym_freq]) -> &'a mut [tdefl_sym_freq]
{
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

// TODO change to iterators
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
                    symbols[next].m_key = symbols[next].m_key + symbols[root].m_key; // TODO why cast to u16 in C?
                    symbols[root].m_key = next as u16;
                    root += 1;
                } else {
                    symbols[next].m_key = symbols[next].m_key + symbols[leaf].m_key;
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

pub fn tdefl_huffman_enforce_max_code_size_oxide(num_codes: &mut [c_int],
                                                 code_list_len: usize,
                                                 max_code_size: usize)
{
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

pub fn tdefl_optimize_huffman_table_oxide(h: &mut HuffmanOxide,
                                          table_num: usize,
                                          table_len: usize,
                                          code_size_limit: usize,
                                          static_table: bool)
{
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

pub fn tdefl_start_dynamic_block_oxide(h: &mut HuffmanOxide, output: &mut OutputBufferOxide) -> io::Result<()> {
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
        pub prev_code_size: u8
    }

    let mut rle = RLE {
        rle_z_count: 0,
        rle_repeat_count: 0,
        prev_code_size: 0xFF
    };

    let tdefl_rle_prev_code_size = |rle: &mut RLE,
                                    packed_code_sizes: &mut Cursor<&mut [u8]>,
                                    h: &mut HuffmanOxide| -> io::Result<()>
        {
            if rle.rle_repeat_count != 0 {
                if rle.rle_repeat_count < 3 {
                    h.count[2][rle.prev_code_size as usize] = (h.count[2][rle.prev_code_size as usize] as i32 + rle.rle_repeat_count as i32) as u16; // TODO
                    while rle.rle_repeat_count != 0 {
                        rle.rle_repeat_count -= 1;
                        packed_code_sizes.write(&[rle.prev_code_size][..])?;
                    }
                } else {
                    h.count[2][16] = (h.count[2][16] as i32 + 1) as u16;
                    packed_code_sizes.write(&[16, (rle.rle_repeat_count as i32 - 3) as u8][..])?;
                }
                rle.rle_repeat_count = 0;
            }

            Ok(())
        };

    let tdefl_rle_zero_code_size = |rle: &mut RLE,
                                    packed_code_sizes: &mut Cursor<&mut [u8]>,
                                    h: &mut HuffmanOxide| -> io::Result<()>
        {
            if rle.rle_z_count != 0 {
                if rle.rle_z_count < 3 {
                    h.count[2][0] = (h.count[2][0] as i32 + rle.rle_z_count as i32) as u16;
                    while rle.rle_z_count != 0 {
                        rle.rle_z_count -= 1;
                        packed_code_sizes.write(&[0][..])?;
                    }
                } else if rle.rle_z_count <= 10 {
                    h.count[2][17] = (h.count[2][17] as i32 + 1) as u16;
                    packed_code_sizes.write(&[17, (rle.rle_z_count as i32 - 3) as u8][..])?;
                } else {
                    h.count[2][18] = (h.count[2][18] as i32 + 1) as u16;
                    packed_code_sizes.write(&[18, (rle.rle_z_count as i32 - 11) as u8][..])?;
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
                h.count[2][code_size as usize] = (h.count[2][code_size as usize] as i32 + 1) as u16; // TODO why as u16?
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

pub fn tdefl_start_static_block_oxide(h: &mut HuffmanOxide, output: &mut OutputBufferOxide) -> io::Result<()> {
    memset(&mut h.code_sizes[0][0..144], 8);
    memset(&mut h.code_sizes[0][144..256], 9);
    memset(&mut h.code_sizes[0][256..280], 7);
    memset(&mut h.code_sizes[0][280..288], 8);

    memset(&mut h.code_sizes[1][..32], 5);

    tdefl_optimize_huffman_table_oxide(h, 0, 288, 15, true);
    tdefl_optimize_huffman_table_oxide(h, 1, 32, 15, true);

    output.put_bits(1, 2)
}

// TODO: only slow version
pub fn tdefl_compress_lz_codes_oxide(h: &mut HuffmanOxide,
                                     output: &mut OutputBufferOxide,
                                     lz_code_buf: &[u8]) -> io::Result<bool>
{
    let mut flags = 1;

    let mut i = 0;
    while i < lz_code_buf.len() {
        if flags == 1 {
            flags = lz_code_buf[i] as u32 | 0x100;
            i += 1;
        }

        if flags & 1 == 1 {
            let sym;
            let num_extra_bits;

            let match_len = lz_code_buf[i] as usize;
            let match_dist = lz_code_buf[i + 1] as usize | ((lz_code_buf[i + 2] as usize) << 8);
            i += 3;

            assert!(h.code_sizes[0][TDEFL_LEN_SYM[match_len] as usize] != 0);
            output.put_bits(h.codes[0][TDEFL_LEN_SYM[match_len] as usize] as u32,
                            h.code_sizes[0][TDEFL_LEN_SYM[match_len] as usize] as u32)?;

            output.put_bits(match_len as u32 & MZ_BITMASKS[TDEFL_LEN_EXTRA[match_len] as usize] as u32,
                            TDEFL_LEN_EXTRA[match_len] as u32)?;

            if match_dist < 512 {
                sym = TDEFL_SMALL_DIST_SYM[match_dist] as usize;
                num_extra_bits = TDEFL_SMALL_DIST_EXTRA[match_dist] as usize;
            } else {
                sym = TDEFL_LARGE_DIST_SYM[match_dist >> 8] as usize;
                num_extra_bits = TDEFL_LARGE_DIST_EXTRA[match_dist >> 8] as usize;
            }

            assert!(h.code_sizes[1][sym] != 0);
            output.put_bits(h.codes[1][sym] as u32, h.code_sizes[1][sym] as u32)?;
            output.put_bits(match_dist as u32 & MZ_BITMASKS[num_extra_bits as usize] as u32, num_extra_bits as u32)?;
        } else {
            let lit = lz_code_buf[i];
            i += 1;

            assert!(h.code_sizes[0][lit as usize] != 0);
            output.put_bits(h.codes[0][lit as usize] as u32, h.code_sizes[0][lit as usize] as u32)?;
        }

        flags >>= 1;
    }

    output.put_bits(h.codes[0][256] as u32, h.code_sizes[0][256] as u32)?;

    Ok(true)
}

pub fn tdefl_compress_block_oxide(h: &mut HuffmanOxide,
                                  output: &mut OutputBufferOxide,
                                  lz: &LZOxide,
                                  static_block: bool) -> io::Result<bool>
{
    if static_block {
        tdefl_start_static_block_oxide(h, output)?;
    } else {
        tdefl_start_dynamic_block_oxide(h, output)?;
    }

    tdefl_compress_lz_codes_oxide(h, output, &lz.codes[..lz.code_position])
}

pub fn tdefl_flush_block_oxide(h: &mut HuffmanOxide,
                               mut output: OutputBufferOxide,
                               lz: &mut LZOxide,
                               dict: &mut DictOxide,
                               p: &mut ParamsOxide,
                               c: &mut CallbackOxide,
                               flush: TDEFLFlush,
                               local_buf: bool) -> io::Result<c_int>
{
    let use_raw_block = (p.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0) &&
        (dict.lookahead_pos - dict.code_buf_dict_pos) <= dict.size;

    assert!(p.flush_remaining == 0);
    p.flush_ofs = 0;
    p.flush_remaining = 0;

    lz.init_flag();

    if p.flags & TDEFL_WRITE_ZLIB_HEADER != 0 && p.block_index == 0 {
        output.put_bits(0x78, 8)?;
        output.put_bits(0x01, 8)?;
    }

    output.put_bits((flush == TDEFLFlush::Finish) as u32, 1)?;

    let saved_buffer = output.save();

    let mut comp_success = false;
    if !use_raw_block {
        let use_static = (p.flags & TDEFL_FORCE_ALL_STATIC_BLOCKS != 0) || (lz.total_bytes < 48);
        comp_success = tdefl_compress_block_oxide(h, &mut output, lz, use_static)?;
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
        tdefl_compress_block_oxide(h, &mut output, lz, true)?;
    }

    if flush != TDEFLFlush::None {
        if flush == TDEFLFlush::Finish {
            output.pad_to_bytes()?;
            if p.flags & TDEFL_WRITE_ZLIB_HEADER != 0 {
                let mut adler = p.adler32;
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

    memset(&mut h.count[0][..TDEFL_MAX_HUFF_SYMBOLS_0], 0);
    memset(&mut h.count[1][..TDEFL_MAX_HUFF_SYMBOLS_1], 0);

    lz.code_position = 1;
    lz.flag_position = 0;
    lz.num_flags_left = 8;
    dict.code_buf_dict_pos += lz.total_bytes;
    lz.total_bytes = 0;
    p.block_index += 1;

    let mut n = output.inner.position();
    if n != 0 {
        match (c.put_buf_func, c.put_buf_user.as_mut()) {
            (Some(callback), Some(user)) => {
                c.in_buf_size.as_mut().map(|size| **size = dict.src_pos);
                let call_success = unsafe {
                    (callback)(&output.inner.get_ref()[0] as *const u8 as *const c_void, n as c_int, *user)
                };

                if !call_success {
                    p.prev_return_status = TDEFLStatus::PutBufFailed;
                    return Ok(p.prev_return_status as c_int);
                }
            },
            _ => {
                if local_buf {
                    let bytes_to_copy = cmp::min(n as usize, **(c.out_buf_size.as_mut().unwrap()) - p.out_buf_ofs);
                    unsafe {
                        ptr::copy_nonoverlapping(&output.inner.get_ref()[0] as *const u8,
                                                 (&mut (c.out_buf.as_mut().unwrap())[p.out_buf_ofs as usize]) as *mut u8,
                                                 bytes_to_copy);
                    }

                    p.out_buf_ofs += bytes_to_copy;
                    n -= bytes_to_copy as u64;
                    if n != 0 {
                        p.flush_ofs = bytes_to_copy as c_uint;
                        p.flush_remaining = n as c_uint;
                    }
                } else {
                    p.out_buf_ofs += n as usize;
                }
            }
        }
    }

    Ok(p.flush_remaining as c_int)
}

// TODO: only slow version
pub fn tdefl_find_match_oxide(dict: &DictOxide,
                              lookahead_pos: c_uint,
                              max_dist: c_uint,
                              max_match_len: c_uint,
                              mut match_dist: c_uint,
                              mut match_len: c_uint) -> (c_uint, c_uint)
{
    assert!(max_match_len as usize <= TDEFL_MAX_MATCH_LEN);

    let pos = lookahead_pos & TDEFL_LZ_DICT_SIZE_MASK;
    let mut probe_pos = pos;
    let mut num_probes_left = dict.max_probes[(match_len >= 32) as usize];

    let mut c0 = dict.dict[(pos + match_len) as usize];
    let mut c1 = dict.dict[(pos + match_len - 1) as usize];

    if max_match_len <= match_len { return (match_dist, match_len) }

    loop {
        let mut dist = 0;
        let mut found = false;
        while !found {
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
                if (dict.dict[(probe_pos + match_len) as usize] == c0) && (dict.dict[(probe_pos + match_len - 1) as usize] == c1) {
                    ProbeResult::Found
                } else {
                    ProbeResult::NotFound
                }
            };

            for _ in 0..3 {
                match tdefl_probe() {
                    ProbeResult::OutOfBounds => return (match_dist, match_len),
                    ProbeResult::Found => { found = true; break },
                    ProbeResult::NotFound => ()
                }
            }
        }

        if dist == 0 { return (match_dist, match_len) }

        let probe_len = dict.dict[pos as usize..].iter().zip(&dict.dict[probe_pos as usize..])
            .take(max_match_len as usize).take_while(|&(&p, &q)| p == q).count() as c_uint;

        if probe_len > match_len {
            match_dist = dist;
            match_len = probe_len;
            if probe_len == max_match_len { return (match_dist, match_len) }

            c0 = dict.dict[(pos + match_len) as usize];
            c1 = dict.dict[(pos + match_len - 1) as usize];
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

pub fn tdefl_record_match_oxide(h: &mut HuffmanOxide,
                                lz: &mut LZOxide,
                                mut match_len: c_uint,
                                mut match_dist: c_uint)
{
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

pub fn tdefl_compress_normal_oxide(h: &mut HuffmanOxide,
                                   lz: &mut LZOxide,
                                   dict: &mut DictOxide,
                                   p: &mut ParamsOxide,
                                   c: &mut CallbackOxide) -> bool
{
    let mut  src_pos = dict.src_pos;
    let mut src_buf_left = p.src_buf_left;

    while src_buf_left != 0 || (p.flush != TDEFLFlush::None && dict.lookahead_size != 0) {
        let num_bytes_to_process = cmp::min(src_buf_left, TDEFL_MAX_MATCH_LEN - dict.lookahead_size as usize);
        if dict.lookahead_size + dict.size >= TDEFL_MIN_MATCH_LEN - 1 {
            let mut dst_pos = (dict.lookahead_pos + dict.lookahead_size) & TDEFL_LZ_DICT_SIZE_MASK;
            let mut ins_pos = dict.lookahead_pos + dict.lookahead_size - 2;
            let mut hash = ((dict.dict[(ins_pos & TDEFL_LZ_DICT_SIZE_MASK) as usize] as c_uint) << TDEFL_LZ_HASH_SHIFT) ^
                (dict.dict[((ins_pos + 1) & TDEFL_LZ_DICT_SIZE_MASK) as usize] as c_uint);

            src_buf_left -= num_bytes_to_process;
            dict.lookahead_size += num_bytes_to_process as c_uint;
            for &c in &c.in_buf.unwrap()[src_pos..src_pos + num_bytes_to_process] {
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
            for &c in &c.in_buf.unwrap()[src_pos..src_pos + num_bytes_to_process] {
                let dst_pos = (dict.lookahead_pos + dict.lookahead_size) & TDEFL_LZ_DICT_SIZE_MASK;
                src_buf_left -= 1;
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
        if p.flush == TDEFLFlush::None && (dict.lookahead_size as usize) < TDEFL_MAX_MATCH_LEN { break }

        let mut len_to_move = 1;
        let mut cur_match_dist = 0;
        let mut cur_match_len = if p.saved_match_len != 0 { p.saved_match_len } else { TDEFL_MIN_MATCH_LEN - 1 };
        let cur_pos = dict.lookahead_pos & TDEFL_LZ_DICT_SIZE_MASK;
        if p.flags & (TDEFL_RLE_MATCHES | TDEFL_FORCE_ALL_RAW_BLOCKS) != 0 {
            if dict.size != 0 && p.flags & TDEFL_FORCE_ALL_RAW_BLOCKS == 0 {
                let c = dict.dict[((cur_pos - 1) & TDEFL_LZ_DICT_SIZE_MASK) as usize];
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
        let filter_small = p.flags & TDEFL_FILTER_MATCHES != 0 && cur_match_len <= 5;
        if far_and_small || filter_small || cur_pos == cur_match_dist {
            cur_match_dist = 0;
            cur_match_len = 0;
        }

        if p.saved_match_len != 0 {
            if cur_match_len > p.saved_match_len {
                tdefl_record_literal_oxide(h, lz, p.saved_lit);
                if cur_match_len >= 128 {
                    tdefl_record_match_oxide(h, lz, cur_match_len, cur_match_dist);
                    p.saved_match_len = 0;
                    len_to_move = cur_match_len;
                } else {
                    p.saved_lit = dict.dict[cur_pos as usize];
                    p.saved_match_dist = cur_match_dist;
                    p.saved_match_len = cur_match_len;
                }
            } else {
                tdefl_record_match_oxide(h, lz, p.saved_match_len, p.saved_match_dist);
                len_to_move = p.saved_match_len - 1;
                p.saved_match_len = 0;
            }
        } else if cur_match_dist == 0 {
            tdefl_record_literal_oxide(h, lz, dict.dict[cmp::min(cur_pos as usize, dict.dict.len() - 1)]);
        } else if p.greedy_parsing || (p.flags & TDEFL_RLE_MATCHES != 0) || cur_match_len >= 128 {
            tdefl_record_match_oxide(h, lz, cur_match_len, cur_match_dist);
            len_to_move = cur_match_len;
        } else {
            p.saved_lit = dict.dict[cmp::min(cur_pos as usize, dict.dict.len() - 1)];
            p.saved_match_dist = cur_match_dist;
            p.saved_match_len = cur_match_len;
        }

        dict.lookahead_pos += len_to_move;
        assert!(dict.lookahead_size >= len_to_move);
        dict.lookahead_size -= len_to_move;
        dict.size = cmp::min(dict.size + len_to_move, TDEFL_LZ_DICT_SIZE as c_uint);

        let lz_buf_tight = lz.code_position > TDEFL_LZ_CODE_BUF_SIZE - 8;
        let raw = p.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0;
        let fat = ((lz.code_position * 115) >> 7) >= lz.total_bytes as usize;
        let fat_or_raw = (lz.total_bytes > 31 * 1024) && (fat || raw);

        if lz_buf_tight || fat_or_raw {
            dict.src_pos = src_pos;
            p.src_buf_left = src_buf_left;

            unsafe {
                (*lz.d).m_out_buf_ofs = p.out_buf_ofs;
            }

            let output = unsafe { OutputBufferOxide::choose_buffer_new(lz.d) };
            let n = tdefl_flush_block_oxide(
                h,
                output.0,
                lz,
                dict,
                p,
                c,
                TDEFLFlush::None,
                output.1
            ).unwrap_or(TDEFLStatus::PutBufFailed as c_int);
            if n != 0 { return n > 0 }
        }
    }

    dict.src_pos = src_pos;
    p.src_buf_left = src_buf_left;
    true
}

pub fn tdefl_get_adler32_oxide(d: &tdefl_compressor) -> c_uint {
    d.m_adler32
}

pub fn tdefl_create_comp_flags_from_zip_params_oxide(level: c_int,
                                                     window_bits: c_int,
                                                     strategy: c_int) -> c_uint
{
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
