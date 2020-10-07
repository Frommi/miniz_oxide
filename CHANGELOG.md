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
