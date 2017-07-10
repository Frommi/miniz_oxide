extern crate gcc;

use std::process::Command;

#[cfg(not(feature="afl_test"))]
fn main() {
    gcc::compile_library("libminiz.a",
                         &["miniz.c", "miniz_zip.c", "miniz_tinfl.c", "miniz_tdef.c"]);
}

#[cfg(feature="afl_test")]
fn main() {
    Command::new("./build_afl.sh").status().unwrap();
    println!("cargo:rustc-link-search=native=bin");
    println!("cargo:rustc-link-lib=static=miniz");
}
