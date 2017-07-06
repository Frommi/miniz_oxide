cargo build --release
cargo build --release
afl-fuzz -i in -o out target/release/miniz_oxide