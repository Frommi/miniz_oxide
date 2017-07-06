#!/bin/bash

set -e

. amalgamate.sh

cargo build --release

g++ tests/miniz_tester.cpp tests/timer.cpp amalgamation/miniz.c -o bin/miniz_tester -I. -ggdb -O2 -L./target/release -lminiz_oxide

for i in 1 2 3 4 5 6
do
    gcc examples/example$i.c amalgamation/miniz.c -o bin/example$i -lm -I. -ggdb -L./target/release -lminiz_oxide
done

mkdir -p test_scratch
if ! test -e "test_scratch/linux-4.8.11"
then
    cd test_scratch
    wget https://cdn.kernel.org/pub/linux/kernel/v4.x/linux-4.8.11.tar.xz -O linux-4.8.11.tar.xz
    tar xf linux-4.8.11.tar.xz
    cd ..
fi

cd test_scratch
../bin/miniz_tester -v a linux-4.8.11
../bin/miniz_tester -v -r a linux-4.8.11
../bin/miniz_tester -v -b -r a linux-4.8.11
../bin/miniz_tester -v -a a linux-4.8.11

mkdir -p large_file
truncate -s 5G large_file/lf
../bin/miniz_tester -v -a a large_file
