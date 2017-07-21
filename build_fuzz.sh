#!/usr/bin/env bash

gcc -c -fPIC miniz/*.c
for f in *.o ; do mv "$f" "c_$f" ; done

for f in c_*.o ; do objcopy "$f" --redefine-syms redefine.txt; done

gcc -c -fPIC miniz_stub/*.c
ar rsc -o bin/libminiz.a *.o

rm *.o
