use super::*;

pub fn tdefl_radix_sort_syms_oxide<'a>(syms0: &'a mut [tdefl_sym_freq],
                                       syms1: &'a mut [tdefl_sym_freq]) -> &'a mut [tdefl_sym_freq] {
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
