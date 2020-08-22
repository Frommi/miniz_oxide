<a name="0.4.1"></a>
### 0.4.1 (2020-08-22)

#### Features

* **inflate:**
  * Add support for limiting output size when decompressing to vec  ([f8c25f3f](f8c25f3f))
  * Introduce reset policy to control InflateState::reset ([1f95a16f](1f95a16f)), closes [#89](89))

* **core:**
  * Add an optional feature for 1.34.2 backwards compatibility ([d18e847d](d18e847d))

<a name="0.4.0"></a>
## 0.4.0 (2020-06-28)


#### Features

* **core:**
  *  Switch from adler32 to adler crate ([ad0f8fef](ad0f8fef))
  *  Flag `miniz_oxide` as a `#![no_std]` library ([7f5aedd7](7f5aedd7))


<a name="0.3.7"></a>
## 0.3.7 (2020-04-30)


#### Bug Fixes

* **deflate:**
  *  overflow panic with very large input buffer ([f0b0e8fd](f0b0e8fd))
  *  compress_to_vec infinite loop ([f3299c8e](f3299c8e), closes [#75](75))
