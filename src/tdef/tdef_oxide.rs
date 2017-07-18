use super::*;

pub fn tdefl_radix_sort_syms_oxide<'a>(syms0: &'a mut [tdefl_sym_freq],
                                       syms1: &'a mut [tdefl_sym_freq]) -> &'a mut [tdefl_sym_freq]
{
    let mut hist = [[0; 256]; 2];

    for freq in syms0.iter() {
        hist[0][(freq.m_key & 0xFF) as usize] += 1;
        hist[1][((freq.m_key >> 8) & 0xFF) as usize] += 1;
    }

    let mut n_passes = 2;
    if syms0.len() == hist[1][0] {
        n_passes -= 1;
    }

    let mut current_syms = syms0;
    let mut new_syms = syms1;

    for pass in 0..n_passes {
        let mut offsets = [0; 256];
        let mut offset = 0;
        for i in 0..256 {
            offsets[i] = offset;
            offset += hist[pass][i];
        }

        for sym in current_syms.iter() {
            let j = ((sym.m_key >> (pass * 8)) & 0xFF) as usize;
            new_syms[offsets[j]] = *sym;
            offsets[j] += 1;
        }

        mem::swap(&mut current_syms, &mut new_syms);
    }

    current_syms
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
    let num_probes = (if level >= 0 { cmp::min(10, level) } else { ::MZ_DEFAULT_LEVEL }) as usize;
    let greedy = if level <= 3 { TDEFL_GREEDY_PARSING_FLAG } else { 0 } as c_uint;
    let mut comp_flags = s_tdefl_num_probes[num_probes] | greedy;

    if window_bits > 0 {
        comp_flags |= TDEFL_WRITE_ZLIB_HEADER as c_uint;
    }

    if level == 0 {
        comp_flags |= TDEFL_FORCE_ALL_RAW_BLOCKS as c_uint;
    } else if strategy == ::MZ_FILTERED {
        comp_flags |= TDEFL_FILTER_MATCHES as c_uint;
    } else if strategy == ::MZ_HUFFMAN_ONLY {
        comp_flags &= !TDEFL_MAX_PROBES_MASK as c_uint;
    } else if strategy == ::MZ_FIXED {
        comp_flags |= TDEFL_FORCE_ALL_STATIC_BLOCKS as c_uint;
    } else if strategy == ::MZ_RLE {
        comp_flags |= TDEFL_RLE_MATCHES as c_uint;
    }

    comp_flags
}
