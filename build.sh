#!/usr/bin/env bash

cd $(dirname $0)
cargo build --release
cp target/release/libminiz_oxide.a .
