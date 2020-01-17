cd wasm
wasm-pack build --target no-modules
cd ..
RUSTFLAGS="-C target-cpu=native" cargo build --release
cat wasm/pkg/wasm.js | sed 's/Object.assign(init/Object.assign({init}/' > static/wasm.js
cp wasm/pkg/wasm_bg.wasm static/
