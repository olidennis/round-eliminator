cd wasm
wasm-pack build --release --target no-modules
cd ..
RUSTFLAGS="-C target-cpu=native" cargo build --release
cat wasm/pkg/wasm.js | sed 's/Object.assign(init/Object.assign({init}/' > static/wasm.js
cp wasm/pkg/wasm_bg.wasm static/
cd static
wasm-opt -O3 -o test.wasm wasm_bg.wasm
mv test.wasm wasm_bg.wasm
wasm-opt -O3 -o test.wasm wasm_bg.wasm
mv test.wasm wasm_bg.wasm
wasm-opt -O3 -o test.wasm wasm_bg.wasm
mv test.wasm wasm_bg.wasm
cd ..
