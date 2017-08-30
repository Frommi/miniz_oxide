#[cfg(any(feature = "fuzzing", feature = "build_non_rust"))]
extern crate gcc;

#[cfg(all(not(feature = "fuzzing"), feature = "build_non_rust"))]
fn main() {
    gcc::compile_library("libminiz.a",
                         &["miniz_stub/miniz.c",
                           "miniz_stub/miniz_zip.c",
                           "miniz_stub/miniz_tinfl.c",
                           "miniz_stub/miniz_tdef.c"]);
}

#[cfg(feature = "fuzzing")]
fn main() {
    use std::process::Command;

    Command::new("./build_fuzz.sh").status().unwrap();
    println!("cargo:rustc-link-search=native=bin");
    println!("cargo:rustc-link-lib=static=miniz");
}

#[cfg(all(not(feature = "fuzzing"), not(feature = "build_non_rust")))]
fn main() {}
