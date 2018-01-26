#!/usr/bin/env bash

# TODO: This is broken at the moment.
cargo fuzz run fuzz_high -- -max_len=900
