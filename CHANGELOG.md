# Changelog

All notable changes to this project will be documented in this file.

---
## [0.8.10](https://github.com/Frommi/miniz_oxide/compare/0.8.9..0.8.10) - 2025-12-03

### Documentation

- **(inflate)** add documentation and doctest for decompress_slice_iter_to_slice - ([97465ca](https://github.com/Frommi/miniz_oxide/commit/97465ca2e441b204d4ccffe3157a0bd975a6765d)) - oyvindln

### Miscellaneous Tasks

- Do not force `std` with `serde` feature ([#178](https://github.com/Frommi/miniz_oxide/issues/178)) - ([4f38d06](https://github.com/Frommi/miniz_oxide/commit/4f38d065596d7f2078d3266c9ab130aec9943c5b)) - clabby
- update ci to run at 1.60 since that is min version now due to serde dep thing - ([cb87f3c](https://github.com/Frommi/miniz_oxide/commit/cb87f3cf57fc02e409abb7db3b6ef1ac1c22a1f9)) - oyvindln

### Other

- Implement support for partial flushes ([#179](https://github.com/Frommi/miniz_oxide/issues/179)) - ([2ede365](https://github.com/Frommi/miniz_oxide/commit/2ede365ff9b7d9eac8162d6022f480473fea2c8f)) - Jonathan Behrens
- Add decompress_with_limit to handle ring buffers ([#183](https://github.com/Frommi/miniz_oxide/issues/183)) - ([bac1abe](https://github.com/Frommi/miniz_oxide/commit/bac1abee827765b7d639ba9c574c28e444d7d3df)) - peckpeck

---
## [0.8.9](https://github.com/Frommi/miniz_oxide/compare/0.8.8..0.8.9) - 2025-06-09

### Other

- Remove `compiler-builtins` from `rustc-dep-of-std` dependencies ([#173](https://github.com/Frommi/miniz_oxide/issues/173)) - ([025c06e](https://github.com/Frommi/miniz_oxide/commit/025c06ed3258817474f8c2608e6decc43ce1de73)) - Trevor Gross

---
## [0.8.8](https://github.com/Frommi/miniz_oxide/compare/0.8.7..0.8.8) - 2025-04-08

### Bug Fixes

- **(inflate)** fix possible `attempt to subtract with overflow` ([#172](https://github.com/Frommi/miniz_oxide/issues/172)) - ([db85297](https://github.com/Frommi/miniz_oxide/commit/db85297b646da470134ab079733d151b197efb87)) - Tymoteusz Kubicz
- **(inflate)** use wrapping instead of saturating in transfer and add test cate for overflow panic in debug mode - ([4ed4535](https://github.com/Frommi/miniz_oxide/commit/4ed45352308f269cd49a1291e191d60f6b84f07c)) - oyvindln
- disable a bunch more stuff that is not used when compiled as part of rustc - ([bf3cca6](https://github.com/Frommi/miniz_oxide/commit/bf3cca66bf4b38f3505f9613cff0f9c0fa6f8514)) - oyvindln
- add fuzz target for running via flate2 - ([adadb9f](https://github.com/Frommi/miniz_oxide/commit/adadb9f53836de14b9b88d71337da36e59f32a24)) - oyvindln

---
## [0.8.7](https://github.com/Frommi/miniz_oxide/compare/0.8.6..0.8.7) - 2025-04-03



### Bug Fixes

- **(inflate)** make block boundary function a feature since it breaks semver and add to test - ([862fb2c](https://github.com/Frommi/miniz_oxide/commit/862fb2c2b8b7847294224a74c8055e98285a80ea)) - oyvindln


---
## [0.8.6](https://github.com/Frommi/miniz_oxide/compare/0.8.5..0.8.6) - 2025-04-03

Yanked as it broke semver due to adding an enum variant - fixed in 0.8.7 by adding the new functionality as an optional feature for now.

### Bug Fixes

- **(deflate)** help the compiler evade two bounds checks to improve compression performance a little - ([633e59f](https://github.com/Frommi/miniz_oxide/commit/633e59fd7efa3fe73be7a712503a9b5ede7ef2c1)) - oyvindln
- **(deflate)** further deflate performance improvements especially on fast mode - ([5a65104](https://github.com/Frommi/miniz_oxide/commit/5a651048f3ef69d65aa28ffb8ecb78229e081dc1)) - oyvindln
- **(docs)** update miniz_oxide readme a bit - ([743ae50](https://github.com/Frommi/miniz_oxide/commit/743ae5065612893ac073ade262bf3a2933702a73)) - oyvindln
- **(inflate)** don't use bit reverse lookup table when not using alloc and make it smaller - ([8e331bb](https://github.com/Frommi/miniz_oxide/commit/8e331bbccae4691d68e1135b87662fb61cdb05da)) - oyvindln
- **(inflate)** correctly return MZError::buf from inflate on incomplete stream - ([061069e](https://github.com/Frommi/miniz_oxide/commit/061069eed84bcb7b7d84ade442fe31b817a3464f)) - oyvindln
- **(inflate)** improve inflate perf in some cases when using wrapping buffer - ([44a3e1b](https://github.com/Frommi/miniz_oxide/commit/44a3e1b682b8fb61511afdd24397eec8c241edd8)) - oyvindln
- **(inflate)** evade more bounds checks in inflate and disable stop on block boundary code when compiled as dep of rustc - ([953a54d](https://github.com/Frommi/miniz_oxide/commit/953a54d6924422e531984737c3a4c158b06d4271)) - oyvindln
- **(inflate)** skip stream module when compiling as part of rustc std as it's not used there - ([073160c](https://github.com/Frommi/miniz_oxide/commit/073160c5f9f972d5245e0152e94a5d855a843a03)) - oyvindln
- rename serde feature, separate serde test - ([eee6524](https://github.com/Frommi/miniz_oxide/commit/eee6524fbbd0e6ad05a16c90a2aa57e65816e7f1)) - oyvindln
- remoe unused serde BigArray implementation sizes and update Readme - ([f73670a](https://github.com/Frommi/miniz_oxide/commit/f73670a193dfdd8ce3ce0dd24125efd6e1a90fcd)) - oyvindln
- Block boundary test and cleanup ([#171](https://github.com/Frommi/miniz_oxide/issues/171)) - ([82ada74](https://github.com/Frommi/miniz_oxide/commit/82ada74738db584f54fe3c1310771d9ffa3cb924)) - Philip Taylor

### Features
- add derive(Serialize, Deserialize) to DecompressorOxide ([#166](https://github.com/Frommi/miniz_oxide/issues/166)) - ([c9e5996](https://github.com/Frommi/miniz_oxide/commit/c9e5996da3179261ed07f76f9d0beff0e5c91b4a)) - dishmaker
- Add API to support random access at block boundaries ([#170](https://github.com/Frommi/miniz_oxide/issues/170)) - ([240bcdd](https://github.com/Frommi/miniz_oxide/commit/240bcdde45bc1befe8a43f9d53b955518da1d152)) - Philip Taylor

---
## [0.8.5](https://github.com/Frommi/miniz_oxide/compare/0.8.4..0.8.5) - 2025-02-21

### Bug Fixes

- **(deflate)** some cleanups and evade a bounds check in compress_lz_codes - ([4c38ff8](https://github.com/Frommi/miniz_oxide/commit/4c38ff8abb3f8ee1f3708f8facd15d1fe9975fbc)) - oyvindln
- **(deflate)** fix bug causing 0 length stored block to be output incorrectly causing corrupt stream - ([3d62e6b](https://github.com/Frommi/miniz_oxide/commit/3d62e6b6b81441b4a1867bf1504672c835654919)) - oyvindln


---
## [0.8.4](https://github.com/Frommi/miniz_oxide/compare/0.8.3..0.8.4) - 2025-02-11

### Bug Fixes

- **(deflate)** work around upstream rust change causing performance regression - ([7014124](https://github.com/Frommi/miniz_oxide/commit/701412465814a5add1b620c82a7c4eafb1936b45)) - oyvindln
- **(doc)** typo on example code ([#162](https://github.com/Frommi/miniz_oxide/issues/162)) - ([2119168](https://github.com/Frommi/miniz_oxide/commit/2119168eeee4ff8a8b12505755611e00fe6b96cc)) - Iván Izaguirre
- **(inflate)** Guard against edge case with invalid match distance wrapping around too far when using wrapping buffer - ([4037fee](https://github.com/Frommi/miniz_oxide/commit/4037fee77fd5811ea10fe62a9c772942b6b72cb1)) - oyvindln
- **(deflate)** Avoid stack overflow when initializing HashBuffers. ([#164](https://github.com/Frommi/miniz_oxide/issues/164)) - ([921bc2c](https://github.com/Frommi/miniz_oxide/commit/921bc2c51e450f22a2a9405a908c64005caa92fe)) - Lukasz Anforowicz

---
## [0.8.3](https://github.com/Frommi/miniz_oxide/compare/0.8.2..0.8.3) - 2025-01-13

### Bug Fixes

- **(bench)** add some basic criterion benchmarks - ([ac03751](https://github.com/Frommi/miniz_oxide/commit/ac03751c43df22b9bb7f47e50b7dbb8fc11ac141)) - oyvindln
- **(deflate)** write directly to output buffer instaed of bit buffer to reduce overhead and improve performance of stored blocks a little - ([97ee3f1](https://github.com/Frommi/miniz_oxide/commit/97ee3f1673b0d8bd88f3abcafb6fe392b086e4b7)) - oyvindln
- **(deflate)** split some code into new module and fix panic in pad_to_bytes from prev commit - ([04973ca](https://github.com/Frommi/miniz_oxide/commit/04973cad7b088868e51fd7970d028dad0ef0c5d0)) - oyvindln
- **(deflate)** move stored level to it's own function and simplify to improve performance - ([1f829d2](https://github.com/Frommi/miniz_oxide/commit/1f829d2574a7842f4d5e5a3ff9c33f249451f79f)) - oyvindln
- **(deflate)** remove no longer needed checks for raw mode in compress_normal and commend out accidentally enabled criterion dev dep - ([f357aa1](https://github.com/Frommi/miniz_oxide/commit/f357aa1462f8370592d2a23214490a7391c9f9de)) - oyvindln
- **(miniz_oxide)** add richgel99 (original miniz author) as author and add copyright info from orig miniz in license files - ([c8a4485](https://github.com/Frommi/miniz_oxide/commit/c8a448500ccd9ab040a244dd7db37702ab9e6449)) - oyvindln

---
## [0.8.2](https://github.com/Frommi/miniz_oxide/compare/0.8.1..0.8.2) - 2024-12-17

### Bug Fixes

- **(deflate)** fix ([#159](https://github.com/Frommi/miniz_oxide/issues/159)) - ([e3536a7](https://github.com/Frommi/miniz_oxide/commit/e3536a779451012db9d6f8d803252a4f30ce6b91)) (fix for bug accidentally introduced in the previous release causing panics in some cases)- Matthew Deville

---
## [0.8.1](https://github.com/Frommi/miniz_oxide/compare/0.8.0..0.8.1) - 2024-12-17

### Bug Fixes

- **(fuzzing)** update fuzzing to work again - ([b7a5908](https://github.com/Frommi/miniz_oxide/commit/b7a5908e1b83bde6b60568f6a67952890ab925a9)) - user
- **(deflate)** use built in fill instead of custom memset function - ([c0662f1](https://github.com/Frommi/miniz_oxide/commit/c0662f11528cbc32291bf91d6caa1890774c2729)) - oyvindln
- **(inflate)** use smaller types in inflate struct, split up huffman table arrays to make struct smaller, make zlib level 0 if using rle, other minor tweaks - ([c5f8f76](https://github.com/Frommi/miniz_oxide/commit/c5f8f761148a3a8a0a7f1b42e698c5e630a8cdf6)) - oyvindln
- **(inflate)** use function instead of lookup table for distance extra bits for tiny space/perf saving and fix clippy warnings - ([9f1fc5e](https://github.com/Frommi/miniz_oxide/commit/9f1fc5e5aeee4ce54be3a766e259b030f3b3cfa9)) - oyvindln
- **(inflate)** use inputwrapper struct instead of iter to simplify input reading and change some data types for performance - ([423bdf8](https://github.com/Frommi/miniz_oxide/commit/423bdf84360c087bea6d3e2b463f3c3a2c1a2867)) - oyvindln
- **(inflate)** don't use lookup table on aarch64 and loong since we have bit rev instruction there, fix clippy warnings and fix conditional in tree_lookup that seemed to break perf - ([083e4b3](https://github.com/Frommi/miniz_oxide/commit/083e4b3e66e9e4e45e7c48a56481d62ee6a78bce)) - oyvindln
- **(inflate)** fill fast lookup table with invalid code value instead of zero so we can avoid check in hot code path givin a small performance boost - ([f73e6a4](https://github.com/Frommi/miniz_oxide/commit/f73e6a4600fbfa795d500d45caef4d48f8c85eff)) - oyvindln
- **(inflate)** skip pointlessly clearing unused huffman code length tree - ([b3b1604](https://github.com/Frommi/miniz_oxide/commit/b3b16048bd459782964f10a23aef63bf058389d5)) - oyvindln
- **(inflate)** use built in fill instead of custom memset function - ([e6ee54e](https://github.com/Frommi/miniz_oxide/commit/e6ee54e82c16ddccb6b55d5a20b8aa5cb4669ca0)) - oyvindln
- **(tests)** change workflow to use rust 1.56.0 - ([7258c06](https://github.com/Frommi/miniz_oxide/commit/7258c064bf39cc124210546d535d82c9c6cd1b5f)) - oyvindln
- **(deflate)** set min window bits in inflate header when using rle - ([02a8857](https://github.com/Frommi/miniz_oxide/commit/02a88571dcc58182df15abb5c1b0410bbd5db428)) - oyvindln
- **(inflate)** Derive Clone for InflateState to allow random-access reads ([#157](https://github.com/Frommi/miniz_oxide/issues/157)) - ([0a33eff](https://github.com/Frommi/miniz_oxide/commit/0a33effd414711b379e01b0613ba5ae85a0e14d0)) - Phil Hord

---
## [0.8.0](https://github.com/Frommi/miniz_oxide/compare/0.7.4..0.8.0) - 2024-08-08

### Major changes

This release changes to using the forked adler2 crate as the original adler crate has not seen any updates in the last 3 years and the repositories have been marked as archived.
The minimum rust version has also been bumped slightly to make room for future improvements.

### Bug Fixes

- **(miniz_oxide)** update edition, make more functions const, fix warning, update to adler2 - ([b212371](https://github.com/Frommi/miniz_oxide/commit/b2123715e2f10f29548b3124b2ea0ce91aad8c27)) - oyvindln

---
## [0.7.4](https://github.com/Frommi/miniz_oxide/compare/0.7.3..0.7.4) - 2024-06-18

### Bug Fixes

- **(miniz_oxide)** simplify init_tree a little and use a smaller lookup table for bit reversal - ([2ba520a](https://github.com/Frommi/miniz_oxide/commit/2ba520a458704e9fd12817fd2e945d869502c59c)) - oyvindln
- **(miniz_oxide)** evade bounds checks in record_match to improve compression performance a little - ([d1de8db](https://github.com/Frommi/miniz_oxide/commit/d1de8dba2e2bbea6452c9a1d78b221a0f41dadd2)) - oyvindln
- **(deflate)** evade a bounds check in deflate for a small perf improvement - ([b4baed3](https://github.com/Frommi/miniz_oxide/commit/b4baed337a70c317c5d6a2fa245bda21f461fc6b)) - oyvindln

### Other

- disable c miniz part in miniz_oxide_c_api of bench - ([2f0a9a3](https://github.com/Frommi/miniz_oxide/commit/2f0a9a3b4f2bc49c44efa3fa9e3afada893ab775)) - oyvindln

---
## [0.7.3](https://github.com/Frommi/miniz_oxide/compare/0.7.2..0.7.3) - 2024-05-17

### Bug Fixes

- **(miniz_oxide)** Fix version specification for simd-adler32 ([#150](https://github.com/Frommi/miniz_oxide/issues/150)) - ([35c71e1](https://github.com/Frommi/miniz_oxide/commit/35c71e1d4b20f03936fb690793103b636f1b0038)) - Daniel Müller
- Fix clippy lints ([#151](https://github.com/Frommi/miniz_oxide/issues/151)) - ([7c758d4](https://github.com/Frommi/miniz_oxide/commit/7c758d4d1cabf24108730ddfdc899b7f62bc2d1d)) - Gnome!
- **(miniz_oxide)** Remove lookup table from rustc-std builds ([#152](https://github.com/Frommi/miniz_oxide/issues/152)) - ([434d9ab](https://github.com/Frommi/miniz_oxide/commit/434d9abff04421355a76b87eb632e3fbab917268)) - Gnome!

---
## [0.7.2](https://github.com/Frommi/miniz_oxide/compare/0.7.1..0.7.2) - 2024-02-03

### Bug Fixes

- **(inflate)** Return MZError::Buf when calling inflate with MZFlush::Finish in line with orig miniz and zlib - ([0f50464](https://github.com/Frommi/miniz_oxide/commit/0f50464c6f7eebdf50942d575db4fbf159436167)) - oyvindln
- **(miniz_oxide)** fix tests when with-alloc is not enabled (running with --no-default-features) and make add test run of it to ci - ([4fd32da](https://github.com/Frommi/miniz_oxide/commit/4fd32da7faa624bfdd47c12e5a0a8587824f65bb)) - oyvindln
- **(miniz_oxide)** fix compiler and clippy warnings - ([657c5b2](https://github.com/Frommi/miniz_oxide/commit/657c5b25760a65cdebe0f66e885d813b488ecd6f)) - oyvindln

### Documentation

- fix typo ([#142](https://github.com/Frommi/miniz_oxide/issues/142)) - ([6e3e813](https://github.com/Frommi/miniz_oxide/commit/6e3e8135895c41e1841c32338d2e0170b80a7e88)) - Brian Donovan

### Performance

- Code size reduction from panic reduction ([#145](https://github.com/Frommi/miniz_oxide/issues/145)) - ([201ef39](https://github.com/Frommi/miniz_oxide/commit/201ef393a84c858c78567a304b18439b3db9279c)) - Kornel
- Optimize match_len == 3 ([#146](https://github.com/Frommi/miniz_oxide/issues/146)) - ([10ff5a0](https://github.com/Frommi/miniz_oxide/commit/10ff5a0824a800d2df688894c904ebfc1b2a2dce)) - Kornel

### Other

- Add a roundtrip fuzz target ([#138](https://github.com/Frommi/miniz_oxide/issues/138)) - ([ee29e37](https://github.com/Frommi/miniz_oxide/commit/ee29e371237b01ddbadd4c0709f2d30447f2430c)) - Sergey "Shnatsel" Davidoff

<!-- generated by git-cliff -->

<a name="0.7.1"></a>
### 0.7.1 (2023-02-02)
* **inflate:**
  * Fix for older versions of Rust (thanks jasonish) ([a65d0751](https://github.com/Frommi/miniz_oxide/commit/a65d0751f83c4e518cfecbf18e19de22692ae2b7))

<a name="0.7.0"></a>
### 0.7.0 (2023-02-01)

Yanked release

<a name="0.6.4"></a>
## 0.6.4 (2023-02-01)

Yanked release due to version requirement bump

* **inflate:**
  * move debug assert condition to if stmt  (thanks connorskees) ([b6d8824a](https://github.com/Frommi/miniz_oxide/commit/b6d8824a316522c292968f807c6455c1ec421bea))

<a name="0.6.3"></a>
## 0.6.3 (2023-02-01)

Yanked release due to version requirement bump

#### Bug Fixes

* **inflate:**
  *  Reject input with too many litlen codes as per spec/zlib ([f4ee585e](https://github.com/Frommi/miniz_oxide/commit/f4ee585eb382eeb7ea1db16458438c637de48349)), closes [#130](https://github.com/Frommi/miniz_oxide/issues/130)
* **deflate:**
  * Remove #\[inline(always)\] from CompressorOxide::default() (thanks jrmuizel) ([c7643aa2](https://github.com/Frommi/miniz_oxide/commit/c7643aa2ddd5c2ca45f6f506b281ee988ea8a296))
#### Features
* **inflate:**
  *  optimize inflate::core::transfer (thanks connorskees) ([dd2fa3e3](https://github.com/Frommi/miniz_oxide/commit/dd2fa3e33c7d8ce1bb8bbd7329a0dd571f4e2df6))
  *  optimize inflate::core::init_tree by precomputing reversed bits (thanks connorskees) ([bf660972](https://github.com/Frommi/miniz_oxide/commit/bf660972bcad23717e811e44b7ad8388ba5f29ec)), closes [#82](https://github.com/Frommi/miniz_oxide/issues/82)

#### Other
  * various typo/doc/ci fixes (thanks LollipopFt, striezel, jarede-dev)

## [0.6.2] - 2022-09-04

### Features

- Add std feature to allow error trait for DecompressError and other stuff later down the line
- impl Display for DecompressError

## [0.6.1] - 2022-08-25

### Bug Fixes

- Make building with --no-default-features actually work

<a name="0.6.0"></a>
## 0.6.0 (2022-08-21)

#### Bug Fixes

* **inflate:**  
  * Fix output size limit handling (thanks Shnatsel) ([c08ac1c6](https://github.com/Frommi/miniz_oxide/commit/c08ac1c60699025fea9c39250a41be54c6267813)), closes [#119](https://github.com/Frommi/miniz_oxide/issues/119))

#### Features

* **inflate:**
  * Return currently decompressed data on failure in decompress_to_vec.. functions ([81796330](https://github.com/Frommi/miniz_oxide/commit/8179633092b032b783333c73395490b6b9243823)), closes [#113](https://github.com/Frommi/miniz_oxide/issues/113))
  * Allow for running without allocator (inflate-only) ([96ad0b80](https://github.com/Frommi/miniz_oxide/commit/96ad0b80e107290be872bf8f010fa21d3ca790fe)), closes [#111](https://github.com/Frommi/miniz_oxide/issues/111))


<a name="0.5.4"></a>
### 0.5.4 (2022-05-30)

#### Bug Fixes

* **inflate:**  Backport fix for [#119](https://github.com/Frommi/miniz_oxide/issues/119)) for 0.5 releases ([7d38417c](https://github.com/Frommi/miniz_oxide/commit/7d38417c8ca18ec0cf38a01f66f7e0776b33a1b1))

<a name="0.5.3"></a>
### 0.5.3 (2022-05-30)

Clippy warnings and doc fixes (thanks @hellow554 and @MichaelMcDonnell)

#### Bug Fixes

* **core:**  Don't use simd-adler32 when building as part of std  ([5869904c](https://github.com/Frommi/miniz_oxide/commit/d3dee8760fd8b390ca079e8a80cb28f71948c379))

<a name="0.5.2"></a>
### 0.5.2 (2021-12-04)


#### Bug Fixes

* **inflate:**  Don't return HasMoreOutput if we are waiting for more input to read adler32 ([5869904c](https://github.com/Frommi/miniz_oxide/commit/5869904c7f789580ace18f7e9084acbcd54a95be))

#### Features

* **inflate:**   Add decompress_slice_iter_to_slice function. ([a359d678](https://github.com/Frommi/miniz_oxide/commit/a359d678c8d83565844ac5075e50c43d67d29879))


<a name="0.5.1"></a>
### 0.5.1 (2021-11-11)

Doc updates and minor refactor

<a name="0.5.0"></a>
## 0.5.0 (2021-11-04)


#### Bug Fixes

* **core:**
  * Don't use autofcg for alloc ([a9bf654f](https://github.com/Frommi/miniz_oxide/commit/a9bf654f6a6756eed0812e612d19137f5c486444), closes [#106](https://github.com/Frommi/miniz_oxide/issues/106))

#### Features

* **core:**
  * Add github actions CI and remove unused feature ([694803aa](https://github.com/Frommi/miniz_oxide/commit/694803aaf0bd4af7d00c08c10881bc121f1fb2c7), closes [#107](https://github.com/Frommi/miniz_oxide/issues/107))
  * Add optional use of simd-adler32 instead of adler behind simd feature flag ([19782aa](https://github.com/Frommi/miniz_oxide/commit/19782aa0833d201eed6bcbc847eefaa741bc2e32))

* **inflate:**
  * add option to ignore and not compute zlib checksum when decompressing ([2e9408ae](https://github.com/Frommi/miniz_oxide/commit/2e9408ae4cad10a00c604a008bd4f8c0704f0ac7), closes [#102](https://github.com/Frommi/miniz_oxide/issues/102))


<a name="0.4.4"></a>
### 0.4.4 (2021-02-25)

#### Features

* **core:**
  * Update adler to 1.0 ([b4c67ba0](https://github.com/Frommi/miniz_oxide/commit/b4c67ba06fc526a012d078efc925494d788b88d0))
  * Mark some internal functions as const ([52ce377a](https://github.com/Frommi/miniz_oxide/commit/28514ec09f0b1ce74bfb2d561de52a6652ce377a))

<a name="0.4.3"></a>
### 0.4.3 (2020-10-07)

#### Features

* **core:**
  * Increase the license options ([78d13b47](https://github.com/Frommi/miniz_oxide/commit/78d13b47))
  * Add forbid unsafe code to build ([80859093](https://github.com/Frommi/miniz_oxide/commit/80859093c1298ff5a97e149d9e3a882ce92fbde5))

#### Bug Fixes

* **core:**
  * in-libstd build attempting to use std ([5a5522de](https://github.com/Frommi/miniz_oxide//commit/5a5522de50129513cb059e73020c8f1aad047200))
  * Run CI on 1.34 ([c6f6cd42](https://github.com/Frommi/miniz_oxide/commit/c6f6cd42e8225ca2f32d6e8fd5a2ce6e99b9c7b1))

<a name="0.4.2"></a>
### 0.4.2 (2020-09-13)

#### Features

* **core:**
  * Add automatic alloc detection ([0c67dc5c](https://github.com/Frommi/miniz_oxide/commit/0c67dc5c))

#### Bug Fixes

* **inflate:**
  * Add missing pub to FullReset's data format ([743d6d37](https://github.com/Frommi/miniz_oxide/commit/743d6d37))

<a name="0.4.1"></a>
### 0.4.1 (2020-08-22)

#### Features

* **inflate:**
  * Add support for limiting output size when decompressing to vec  ([f8c25f3f](https://github.com/Frommi/miniz_oxide/commit/f8c25f3f))
  * Introduce reset policy to control InflateState::reset ([1f95a16f](https://github.com/Frommi/miniz_oxide/commit/1f95a16f)), closes [#89](https://github.com/Frommi/miniz_oxide/issues/89))

* **core:**
  * Add an optional feature for 1.34.2 backwards compatibility ([d18e847d](https://github.com/Frommi/miniz_oxide/commit/d18e847d))

<a name="0.4.0"></a>
## 0.4.0 (2020-06-28)


#### Features

* **core:**
  *  Switch from adler32 to adler crate ([ad0f8fef](https://github.com/Frommi/miniz_oxide/commit/ad0f8fef))
  *  Flag `miniz_oxide` as a `#![no_std]` library ([7f5aedd7](https://github.com/Frommi/miniz_oxide/commit/7f5aedd7))


<a name="0.3.7"></a>
## 0.3.7 (2020-04-30)


#### Bug Fixes

* **deflate:**
  *  overflow panic with very large input buffer ([f0b0e8fd](https://github.com/Frommi/miniz_oxide/commit/f0b0e8fd))
  *  compress_to_vec infinite loop ([f3299c8e](https://github.com/Frommi/miniz_oxide/commit/f3299c8e), closes [#75](https://github.com/Frommi/miniz_oxide/issues/75))
