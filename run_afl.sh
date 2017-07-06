#!/usr/bin/env bash

cargo build --release --features=afl_test
afl-fuzz -i in -o out target/release/miniz_oxide
