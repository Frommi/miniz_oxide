#!/usr/bin/env bash

cargo install cargo-benchcmp
cargo bench > benchout
cargo benchcmp miniz:: oxide:: benchout
