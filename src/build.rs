#[cfg(feature = "build_stub_miniz")]
extern crate cc;

#[cfg(not(any(feature = "build_stub_miniz", feature = "build_orig_miniz")))]
fn main() {}

#[cfg(feature = "build_stub_miniz")]
fn main() {
    cc::Build::new()
        .files(
            &[
                "miniz_stub/miniz.c",
                "miniz_stub/miniz_zip.c",
                "miniz_stub/miniz_tinfl.c",
                "miniz_stub/miniz_tdef.c",
            ],
        )
        .compile("libminiz.a");
}

#[cfg(feature = "build_orig_miniz")]
fn main() {
    use std::process::Command;

    Command::new("./build_orig_miniz.sh").status().unwrap();
    println!("cargo:rustc-link-search=native=bin");
    println!("cargo:rustc-link-lib=static=miniz");
}
