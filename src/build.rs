extern crate gcc;

use std::env;

fn main() {
    gcc::compile_library("libminiz.a",
                         &["miniz.c", "miniz_zip.c", "miniz_tinfl.c", "miniz_tdef.c"]);
}
