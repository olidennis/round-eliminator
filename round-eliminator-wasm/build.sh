#!/bin/sh

set -ex

wasm-pack build --out-dir ../www/pkg --target no-modules

cd ../www/pkg
for i in `seq 1 5`; do
    wasm-opt -O3 -o test.wasm round_eliminator_wasm_bg.wasm
    mv test.wasm round_eliminator_wasm_bg.wasm
done