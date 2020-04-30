<a name="0.3.7"></a>
## 0.3.7 (2020-04-30)


#### Bug Fixes

* **deflate:**
  *  overflow panic with very large input buffer ([f0b0e8fd](f0b0e8fd))
  *  compress_to_vec infinite loop ([f3299c8e](f3299c8e), closes [#75](75))
