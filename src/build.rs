extern crate gcc;

use std::env;

fn main() {
    gcc::compile_library("libminiz.a",
                         &["miniz.c", "miniz_zip.c", "miniz_tinfl.c", "miniz_tdef.c"]);
    println!("cargo:root={}", env::var("OUT_DIR").unwrap());
}
