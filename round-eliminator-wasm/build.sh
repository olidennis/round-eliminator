#!/bin/sh

set -ex

wasm-pack build --out-dir ../www/pkg --target no-modules 

cd ../www/pkg
for i in `seq 1 5`; do
    wasm-opt -O3 -o test.wasm round_eliminator_wasm_bg.wasm
    mv test.wasm round_eliminator_wasm_bg.wasm
done

## in this folder:
# cargo build --target wasm32-unknown-unknown     
# wasm-bindgen --target no-modules --out-dir ../www/pkg --keep-debug target/wasm32-unknown-unknown/debug/round_eliminator_wasm.wasm
# cargo wasm2map ../www/pkg/round_eliminator_wasm_bg.wasm
## in ../www
# npx serve
# npx source-map-cli resolve pkg/round_eliminator_wasm_bg.wasm.map 1 wasm-line-where-it-crashed
# OR: just let it crash on chrome, it writes the stacktrace