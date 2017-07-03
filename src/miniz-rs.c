#include "miniz.h"

mz_ulong mz_adler32(mz_ulong adler, const unsigned char *ptr, size_t buf_len) {
    mz_uint32 i, s1 = (mz_uint32)(adler & 0xffff), s2 = (mz_uint32)(adler >> 16);
    size_t block_len = buf_len % 5552;
    if (!ptr)
        return MZ_ADLER32_INIT;
    while (buf_len)
    {
        for (i = 0; i + 7 < block_len; i += 8, ptr += 8)
        {
            s1 += ptr[0], s2 += s1;
            s1 += ptr[1], s2 += s1;
            s1 += ptr[2], s2 += s1;
            s1 += ptr[3], s2 += s1;
            s1 += ptr[4], s2 += s1;
            s1 += ptr[5], s2 += s1;
            s1 += ptr[6], s2 += s1;
            s1 += ptr[7], s2 += s1;
        }
        for (; i < block_len; ++i)
            s1 += *ptr++, s2 += s1;
        s1 %= 65521U, s2 %= 65521U;
        buf_len -= block_len;
        block_len = 5552;
    }
    return (s2 << 16) + s1;
}
