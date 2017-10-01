#[cfg(not(feature = "build_orig_miniz"))]
extern crate cc;

#[cfg(not(feature = "build_orig_miniz"))]
fn main() {
    //    panic!("only stub");
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
    //    panic!("old fuzzy pushover");
    use std::process::Command;

    Command::new("./build_orig_miniz.sh").status().unwrap();
    println!("cargo:rustc-link-search=native=bin");
    println!("cargo:rustc-link-lib=static=miniz");
}
