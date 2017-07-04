#[no_mangle]
pub unsafe extern fn mz_adler32(
    mut adler : usize, mut ptr : *const u8, mut buf_len : usize
) -> usize {
    let mut i : u32;
    let mut s1 : u32 = (adler & 0xffffusize) as (u32);
    let mut s2 : u32 = (adler >> 16i32) as (u32);
    let mut block_len : usize = buf_len.wrapping_rem(5552usize);
    if ptr.is_null() {
        1usize
    } else {
        'loop1: loop {
            if buf_len == 0 {
                break;
            }
            i = 0u32;
            'loop4: loop {
                if !(i.wrapping_add(7u32) as (usize) < block_len) {
                    break;
                }
                s1 = s1.wrapping_add(*ptr.offset(0isize) as (u32));
                s2 = s2.wrapping_add(s1);
                s1 = s1.wrapping_add(*ptr.offset(1isize) as (u32));
                s2 = s2.wrapping_add(s1);
                s1 = s1.wrapping_add(*ptr.offset(2isize) as (u32));
                s2 = s2.wrapping_add(s1);
                s1 = s1.wrapping_add(*ptr.offset(3isize) as (u32));
                s2 = s2.wrapping_add(s1);
                s1 = s1.wrapping_add(*ptr.offset(4isize) as (u32));
                s2 = s2.wrapping_add(s1);
                s1 = s1.wrapping_add(*ptr.offset(5isize) as (u32));
                s2 = s2.wrapping_add(s1);
                s1 = s1.wrapping_add(*ptr.offset(6isize) as (u32));
                s2 = s2.wrapping_add(s1);
                s1 = s1.wrapping_add(*ptr.offset(7isize) as (u32));
                s2 = s2.wrapping_add(s1);
                i = i.wrapping_add(8u32);
                ptr = ptr.offset(8isize);
            }
            'loop5: loop {
                if !(i as (usize) < block_len) {
                    break;
                }
                s1 = s1.wrapping_add(
                         *{
                              let _old = ptr;
                              ptr = ptr.offset(1isize);
                              _old
                          } as (u32)
                     );
                s2 = s2.wrapping_add(s1);
                i = i.wrapping_add(1u32);
            }
            s1 = s1.wrapping_rem(65521u32);
            s2 = s2.wrapping_rem(65521u32);
            buf_len = buf_len.wrapping_sub(block_len);
            block_len = 5552usize;
        }
        (s2 << 16i32).wrapping_add(s1) as (usize)
    }
}


// extern crate libc;

// use libc::*;

// #[no_mangle]
// pub unsafe extern "C" fn mz_adler32(adler: c_ulong, ptr: *const uint8_t, mut buf_len: size_t) -> c_ulong {
//     // let mut s1 = (adler & 0xffff) as u32;
//     // let mut s2 = (adler >> 16) as u32;
//     0
// }
