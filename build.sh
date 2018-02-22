#!/usr/bin/env bash

cd $(dirname $0)

OLD="\#CRATE_TYPE"
NEW="crate-type = \['staticlib', 'rlib'\]"

# Tell cargo that we want a static library to link with.
# --crate-type=staticlib doesn't seem to work, so we modify
# Cargo.toml temoprarily instead.
sed -i "s/$OLD/$NEW/g" Cargo.toml

rm -f libminiz_oxide_c_api.a

if [[ ($# == 0 || $1 == "--release" ) ]]; then
    RUSTFLAGS="-g" cargo build --release --features=miniz_zip -- || exit 1
    cp target/release/libminiz_oxide_c_api.a .
elif [[ $1 == "--debug" ]]; then
    cargo build --features=miniz_zip || exit 1
    cp target/debug/libminiz_oxide_c_api.a .
else
    echo --relese or --debug
fi

sed -i "s/$NEW/$OLD/g" Cargo.toml
