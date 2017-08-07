#!/usr/bin/env bash

cd $(dirname $0)

OLD="crate-type = \['rlib'\]"
NEW="crate-type = \['staticlib', 'rlib'\]"

sed -i "s/$OLD/$NEW/g" Cargo.toml

if [[ ($# == 0 || $1 == "--release" ) ]]; then
#    cargo rustc --release -- --emit asm
    cargo build --release || exit 1
    cp target/release/libminiz_oxide.a .
elif [[ $1 == "--debug" ]]; then
    cargo build || exit 1
    cp target/debug/libminiz_oxide.a .
else
    echo --relese or --debug
fi
