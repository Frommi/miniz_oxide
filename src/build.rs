#[cfg(all(not(feature = "fuzzing"), not(feature = "bench"), feature = "build_non_rust"))]
extern crate gcc;

#[cfg(all(not(feature = "fuzzing"), not(feature = "bench"), feature = "build_non_rust"))]
fn main() {
    gcc::Build::new()
        .files(&[
            "miniz_stub/miniz.c",
            "miniz_stub/miniz_zip.c",
            "miniz_stub/miniz_tinfl.c",
            "miniz_stub/miniz_tdef.c",
        ])
        .compile("libminiz.a");
}

#[cfg(any(feature = "fuzzing", feature = "bench"))]
fn main() {
    use std::process::Command;

    Command::new("./build_fuzz.sh").status().unwrap();
    println!("cargo:rustc-link-search=native=bin");
    println!("cargo:rustc-link-lib=static=miniz");
}

#[cfg(all(not(feature = "fuzzing"), not(feature = "build_non_rust")))]
fn main() {}
