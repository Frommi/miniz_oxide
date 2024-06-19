/* This library (excluding the miniz C code used for tests) is licensed under the MIT license. The library is based on the miniz C library, of which the parts used are dual-licensed under the MIT license and also the unlicense. The parts of miniz that are not covered by the unlicense is some Zip64 code which is only MIT licensed. This and other Zip functionality in miniz is not part of the miniz_oxidde and miniz_oxide_c_api rust libraries.*/

#pragma once

/* Generated with cbindgen:0.20.0 */

/* DO NOT MODIFY THIS MANUALLY! This file was generated using cbindgen.
 * To generate this file:
 *   1. Get the latest cbindgen using `cargo install --force cbindgen`
 *      a. Alternatively, you can clone `https://github.com/eqrion/cbindgen` and use a tagged release
 *   2. Run `rustup run nightly cbindgen toolkit/library/rust/ --lockfile Cargo.lock --crate miniz_oxide_c_api -o miniz_h/miniz.h`
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include "miniz_extra_defs.h"

#define MZ_DEFLATED 8

#define MZ_CRC32_INIT 0

/**
 * Size of the buffer of lz77 encoded data.
 */
#define LZ_CODE_BUF_SIZE (64 * 1024)

/**
 * Size of the output buffer.
 */
#define OUT_BUF_SIZE ((LZ_CODE_BUF_SIZE * 13) / 10)

#define LZ_DICT_FULL_SIZE (((LZ_DICT_SIZE + MAX_MATCH_LEN) - 1) + 1)

/**
 * Size of hash values in the hash chains.
 */
#define LZ_HASH_BITS 15

/**
 * How many bits to shift when updating the current hash value.
 */
#define LZ_HASH_SHIFT ((LZ_HASH_BITS + 2) / 3)

/**
 * Size of the chained hash tables.
 */
#define LZ_HASH_SIZE (1 << LZ_HASH_BITS)

/**
 * Whether to use a zlib wrapper.
 */
#define TDEFL_WRITE_ZLIB_HEADER 4096

/**
 * Should we compute the adler32 checksum.
 */
#define TDEFL_COMPUTE_ADLER32 8192

/**
 * Should we use greedy parsing (as opposed to lazy parsing where look ahead one or more
 * bytes to check for better matches.)
 */
#define TDEFL_GREEDY_PARSING_FLAG 16384

/**
 * Used in miniz to skip zero-initializing hash and dict. We don't do this here, so
 * this flag is ignored.
 */
#define TDEFL_NONDETERMINISTIC_PARSING_FLAG 32768

/**
 * Only look for matches with a distance of 0.
 */
#define TDEFL_RLE_MATCHES 65536

/**
 * Only use matches that are at least 6 bytes long.
 */
#define TDEFL_FILTER_MATCHES 131072

/**
 * Force the compressor to only output static blocks. (Blocks using the default huffman codes
 * specified in the deflate specification.)
 */
#define TDEFL_FORCE_ALL_STATIC_BLOCKS 262144

/**
 * Force the compressor to only output raw/uncompressed blocks.
 */
#define TDEFL_FORCE_ALL_RAW_BLOCKS 524288

#define TINFL_LZ_DICT_SIZE 32768

/**
 * Should we try to parse a zlib header?
 *
 * If unset, [`decompress()`] will expect an RFC1951 deflate stream.  If set, it will expect an
 * RFC1950 zlib wrapper around the deflate stream.
 */
#define TINFL_FLAG_PARSE_ZLIB_HEADER 1

/**
 * There will be more input that hasn't been given to the decompressor yet.
 *
 * This is useful when you want to decompress what you have so far,
 * even if you know there is probably more input that hasn't gotten here yet (_e.g._, over a
 * network connection).  When [`decompress()`][super::decompress] reaches the end of the input
 * without finding the end of the compressed stream, it will return
 * [`TINFLStatus::NeedsMoreInput`][super::TINFLStatus::NeedsMoreInput] if this is set,
 * indicating that you should get more data before calling again.  If not set, it will return
 * [`TINFLStatus::FailedCannotMakeProgress`][super::TINFLStatus::FailedCannotMakeProgress]
 * suggesting the stream is corrupt, since you claimed it was all there.
 */
#define TINFL_FLAG_HAS_MORE_INPUT 2

/**
 * The output buffer should not wrap around.
 */
#define TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF 4

/**
 * Calculate the adler32 checksum of the output data even if we're not inflating a zlib stream.
 *
 * If [`TINFL_FLAG_IGNORE_ADLER32`] is specified, it will override this.
 *
 * NOTE: Enabling/disabling this between calls to decompress will result in an incorrect
 * checksum.
 */
#define TINFL_FLAG_COMPUTE_ADLER32 8

/**
 * Ignore adler32 checksum even if we are inflating a zlib stream.
 *
 * Overrides [`TINFL_FLAG_COMPUTE_ADLER32`] if both are enabled.
 *
 * NOTE: This flag does not exist in miniz as it does not support this and is a
 * custom addition for miniz_oxide.
 *
 * NOTE: Should not be changed from enabled to disabled after decompression has started,
 * this will result in checksum failure (outside the unlikely event where the checksum happens
 * to match anyway).
 */
#define TINFL_FLAG_IGNORE_ADLER32 64

#define MZ_ADLER32_INIT 1

#define MZ_DEFAULT_WINDOW_BITS 15

typedef enum CAPICompressionLevel {
  MZ_NO_COMPRESSION = 0,
  MZ_BEST_SPEED = 1,
  MZ_BEST_COMPRESSION = 9,
  MZ_UBER_COMPRESSION = 10,
  MZ_DEFAULT_LEVEL = 6,
  MZ_DEFAULT_COMPRESSION = -1,
} CAPICompressionLevel;

typedef enum CAPICompressionStrategy {
  MZ_DEFAULT_STRATEGY = 0,
  MZ_FILTERED = 1,
  MZ_HUFFMAN_ONLY = 2,
  MZ_RLE = 3,
  MZ_FIXED = 4,
} CAPICompressionStrategy;

/**
 * Deflate flush modes.
 */
typedef enum CAPIFlush {
  MZ_NO_FLUSH = 0,
  MZ_PARTIAL_FLUSH = 1,
  MZ_SYNC_FLUSH = 2,
  MZ_FULL_FLUSH = 3,
  MZ_FINISH = 4,
  MZ_BLOCK = 5,
} CAPIFlush;

typedef enum CAPIReturnStatus {
  MZ_PARAM_ERROR = -10000,
  MZ_VERSION_ERROR = -6,
  MZ_BUF_ERROR = -5,
  MZ_MEM_ERROR = -4,
  MZ_DATA_ERROR = -3,
  MZ_STREAM_ERROR = -2,
  MZ_ERRNO = -1,
  MZ_OK = 0,
  MZ_STREAM_END = 1,
  MZ_NEED_DICT = 2,
} CAPIReturnStatus;

/**
 * Enum to keep track of what type the internal state is when moving over the C API boundary.
 */
typedef enum StateTypeEnum {
  None = 0,
  InflateType,
  DeflateType,
} StateTypeEnum;

typedef enum tdefl_flush {
  TDEFL_NO_FLUSH = 0,
  TDEFL_SYNC_FLUSH = 2,
  TDEFL_FULL_FLUSH = 3,
  TDEFL_FINISH = 4,
} tdefl_flush;

typedef enum tdefl_status {
  TDEFL_STATUS_BAD_PARAM = -2,
  TDEFL_STATUS_PUT_BUF_FAILED = -1,
  TDEFL_STATUS_OKAY = 0,
  TDEFL_STATUS_DONE = 1,
} tdefl_status;

typedef enum tinfl_status {
  TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS = -4,
  TINFL_STATUS_BAD_PARAM = -3,
  TINFL_STATUS_ADLER32_MISMATCH = -2,
  TINFL_STATUS_FAILED = -1,
  TINFL_STATUS_DONE = 0,
  TINFL_STATUS_NEEDS_MORE_INPUT = 1,
  TINFL_STATUS_HAS_MORE_OUTPUT = 2,
} tinfl_status;

/**
 * Main compression struct. Not the same as `CompressorOxide`
 * #[repr(C)]
 */
typedef struct tdefl_compressor tdefl_compressor;

/**
 * Main decompression struct.
 *
 */
typedef struct DecompressorOxide DecompressorOxide;

typedef struct InternalState InternalState;

typedef void *(*mz_alloc_callback)(void*, size_t, size_t);

typedef void (*mz_free_callback)(void*, void*);

/**
 * Inner stream state containing pointers to the used buffers and internal state.
 */
typedef struct mz_stream {
  /**
   * Pointer to the current start of the input buffer.
   */
  const uint8_t *next_in;
  /**
   * Length of the input buffer.
   */
  unsigned int avail_in;
  /**
   * The total number of input bytes consumed so far.
   */
  unsigned long total_in;
  /**
   * Pointer to the current start of the output buffer.
   */
  uint8_t *next_out;
  /**
   * Space in the output buffer.
   */
  unsigned int avail_out;
  /**
   * The total number of bytes output so far.
   */
  unsigned long total_out;
  const char *msg;
  /**
   * Compressor or decompressor, if it exists.
   * This is boxed to work with the current C API.
   */
  struct InternalState *state;
  /**
   * Allocation function to use for allocating the internal compressor/decompressor.
   * Uses `mz_default_alloc_func` if set to `None`.
   */
  mz_alloc_callback zalloc;
  /**
   * Free function to use for allocating the internal compressor/decompressor.
   * Uses `mz_default_free_func` if `None`.
   */
  mz_free_callback zfree;
  /**
   * Extra data to provide the allocation/deallocation functions.
   * (Not used for the default ones)
   */
  void *opaque;
  /**
   * Whether the stream contains a compressor or decompressor.
   */
  enum StateTypeEnum data_type;
  /**
   * Adler32 checksum of the data that has been compressed or uncompressed.
   */
  unsigned long adler;
  /**
   * Reserved
   */
  unsigned long reserved;
} mz_stream;

typedef int32_t (*tdefl_put_buf_func_ptr)(const void*, int, void*);

typedef struct tinfl_decompressor {
  struct DecompressorOxide *inner;
} tinfl_decompressor;

/**
 * Signature of function used to allocate the compressor/decompressor structs.
 */
typedef void *(*mz_alloc_func)(void*, size_t, size_t);

/**
 * Signature of function used to free the compressor/decompressor structs.
 */
typedef void (*mz_free_func)(void*, void*);

typedef void *(*mz_realloc_func)(void*, void*, size_t, size_t);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

int mz_deflate(struct mz_stream *stream, int flush);

int mz_deflateEnd(struct mz_stream *stream);

int mz_deflateReset(struct mz_stream *stream);

int mz_inflate(struct mz_stream *stream, int flush);

int mz_inflateEnd(struct mz_stream *stream);

int mz_deflateInit(struct mz_stream *stream, int level);

int mz_deflateInit2(struct mz_stream *stream,
                    int level,
                    int method,
                    int window_bits,
                    int mem_level,
                    int strategy);

int mz_inflateInit2(struct mz_stream *stream, int window_bits);

int mz_compress(uint8_t *dest,
                unsigned long *dest_len,
                const uint8_t *source,
                unsigned long source_len);

int mz_compress2(uint8_t *dest,
                 unsigned long *dest_len,
                 const uint8_t *source,
                 unsigned long source_len,
                 int level);

unsigned long mz_deflateBound(struct mz_stream *_stream, unsigned long source_len);

int mz_inflateInit(struct mz_stream *stream);

int mz_uncompress(uint8_t *dest,
                  unsigned long *dest_len,
                  const uint8_t *source,
                  unsigned long source_len);

unsigned long mz_compressBound(unsigned long source_len);

enum tdefl_status tdefl_compress(struct tdefl_compressor *d,
                                 const void *in_buf,
                                 uintptr_t *in_size,
                                 void *out_buf,
                                 uintptr_t *out_size,
                                 enum tdefl_flush flush);

enum tdefl_status tdefl_compress_buffer(struct tdefl_compressor *d,
                                        const void *in_buf,
                                        uintptr_t in_size,
                                        enum tdefl_flush flush);

/**
 * Allocate a compressor.
 *
 * This does initialize the struct, but not the inner constructor,
 * tdefl_init has to be called before doing anything with it.
 */
struct tdefl_compressor *tdefl_allocate(void);

/**
 * Deallocate the compressor. (Does nothing if the argument is null).
 *
 * This also calles the compressor's destructor, freeing the internal memory
 * allocated by it.
 */
void tdefl_deallocate(struct tdefl_compressor *c);

/**
 * Initialize the compressor struct in the space pointed to by `d`.
 * if d is null, an error is returned.
 *
 * Deinitialization is handled by tdefl_deallocate, and thus
 * Compressor should not be allocated or freed manually, but only through
 * tdefl_allocate and tdefl_deallocate
 */
enum tdefl_status tdefl_init(struct tdefl_compressor *d,
                             tdefl_put_buf_func_ptr put_buf_func,
                             void *put_buf_user,
                             int flags);

enum tdefl_status tdefl_get_prev_return_status(struct tdefl_compressor *d);

unsigned int tdefl_get_adler32(struct tdefl_compressor *d);

int tdefl_compress_mem_to_output(const void *buf,
                                 uintptr_t buf_len,
                                 tdefl_put_buf_func_ptr put_buf_func,
                                 void *put_buf_user,
                                 int flags);

void *tdefl_compress_mem_to_heap(const void *src_buf,
                                 uintptr_t src_buf_len,
                                 uintptr_t *out_len,
                                 int flags);

uintptr_t tdefl_compress_mem_to_mem(void *out_buf,
                                    uintptr_t out_buf_len,
                                    const void *src_buf,
                                    uintptr_t src_buf_len,
                                    int flags);

unsigned int tdefl_create_comp_flags_from_zip_params(int level, int window_bits, int strategy);

int32_t tinfl_decompress(struct tinfl_decompressor *r,
                         const uint8_t *in_buf,
                         uintptr_t *in_buf_size,
                         uint8_t *out_buf_start,
                         uint8_t *out_buf_next,
                         uintptr_t *out_buf_size,
                         uint32_t flags);

size_t tinfl_decompress_mem_to_mem(void *p_out_buf,
                                   size_t out_buf_len,
                                   const void *p_src_buf,
                                   size_t src_buf_len,
                                   int flags);

/**
 * Decompress data from `p_src_buf` to a continuously growing heap-allocated buffer.
 *
 * Sets `p_out_len` to the length of the returned buffer.
 * Returns `ptr::null()` if decompression or allocation fails.
 * The buffer should be freed with `miniz_def_free_func`.
 */
void *tinfl_decompress_mem_to_heap(const void *p_src_buf,
                                   size_t src_buf_len,
                                   size_t *p_out_len,
                                   int flags);

/**
 * Allocate a compressor.
 *
 * This does initialize the struct, but not the inner constructor,
 * tdefl_init has to be called before doing anything with it.
 */
struct tinfl_decompressor *tinfl_decompressor_alloc(void);

/**
 * Deallocate the compressor. (Does nothing if the argument is null).
 *
 * This also calles the compressor's destructor, freeing the internal memory
 * allocated by it.
 */
void tinfl_decompressor_free(struct tinfl_decompressor *c);

void tinfl_init(struct tinfl_decompressor *c);

int tinfl_get_adler32(struct tinfl_decompressor *c);

/**
 * Default allocation function using `malloc`.
 */
void *miniz_def_alloc_func(void *_opaque, size_t items, size_t size);

/**
 * Default free function using `free`.
 */
void miniz_def_free_func(void *_opaque, void *address);

void *miniz_def_realloc_func(void *_opaque, void *address, size_t items, size_t size);

/**
 * Calculate adler32 checksum of the provided buffer with the initial adler32 checksum of `adler`.
 * If c_ulong is wider than 32 bits, only the lower 32 bits will be used.
 *
 * Returns MZ_ADLER32_INIT if ptr is `ptr::null`.
 */
unsigned long mz_adler32(unsigned long adler, const uint8_t *ptr, uintptr_t buf_len);

/**
 * Calculate crc-32 of the provided buffer with the initial CRC32 checksum of `crc`.
 * If c_ulong is wider than 32 bits, only the lower 32 bits will be used.
 *
 * Returns MZ_CRC32_INIT if ptr is `ptr::null`.
 */
unsigned long mz_crc32(unsigned long crc, const uint8_t *ptr, size_t buf_len);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus
