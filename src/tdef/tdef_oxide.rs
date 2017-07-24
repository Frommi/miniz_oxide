use super::*;

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

pub fn tdefl_get_adler32_oxide(d: &tdefl_compressor) -> c_uint {
    d.m_adler32
}

#[allow(bad_style)]
const s_tdefl_num_probes: [c_uint; 11] = [0, 1, 6, 32, 16, 32, 128, 256, 512, 768, 1500];

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
    let mut comp_flags = s_tdefl_num_probes[num_probes] | greedy;

    if window_bits > 0 {
        comp_flags |= TDEFL_WRITE_ZLIB_HEADER as c_uint;
    }

    if level == 0 {
        comp_flags |= TDEFL_FORCE_ALL_RAW_BLOCKS as c_uint;
    } else if strategy == ::CompressionStrategy::Filtered as c_int {
        comp_flags |= TDEFL_FILTER_MATCHES as c_uint;
    } else if strategy == ::CompressionStrategy::HuffmanOnly as c_int {
        comp_flags &= !TDEFL_MAX_PROBES_MASK as c_uint;
    } else if strategy == ::CompressionStrategy::Fixed as c_int {
        comp_flags |= TDEFL_FORCE_ALL_STATIC_BLOCKS as c_uint;
    } else if strategy == ::CompressionStrategy::RLE as c_int {
        comp_flags |= TDEFL_RLE_MATCHES as c_uint;
    }

    comp_flags
}
