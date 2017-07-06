#!/usr/bin/env bash

cargo build --release
cp target/release/libminiz_oxide.a .
