#!/usr/bin/env bash

afl-gcc -c -fPIC miniz.c miniz_tdef.c miniz_zip.c miniz_tinfl.c
ar rsc -o bin/libminiz.a miniz.o miniz_tdef.o miniz_zip.o miniz_tinfl.o
rm miniz.o miniz_tdef.o miniz_zip.o miniz_tinfl.o
