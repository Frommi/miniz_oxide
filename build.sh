#!/usr/bin/env bash

cd $(dirname $0)

OLD="crate-type = \['rlib'\]"
NEW="crate-type = \['staticlib', 'rlib'\]"

sed -i "s/$OLD/$NEW/g" Cargo.toml

cargo build --release
cp target/release/libminiz_oxide.a .
