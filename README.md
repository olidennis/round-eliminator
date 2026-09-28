# Round Eliminator 3

This is a new implementation on the `re3` branch. The first feature is Start:
Rust parses and validates an input-free active/passive problem, and the GUI
displays the returned definition. Algorithms and computed properties are not
implemented yet; see [FEATURES.md](FEATURES.md).

The root is a virtual Cargo workspace. `crates/core` contains the portable
problem model, parser, and request handler; `crates/server` and `crates/wasm`
are thin delivery adapters. `web` contains the Vue interface.

## Constraint text

Each nonempty line is an allowed configuration. Whitespace separates parts.
One-character labels can be written together as choices, as in `AB`; use
parentheses for a longer label, as in `(red)(blue)`. A suffix `^n` repeats a
part `n` times. For example, `M U^9` has exact degree 10. Lines of different
degrees are accepted, including on one side. Starred configurations are
currently rejected.

## Run

From this directory:

```sh
cargo test --workspace
wasm-pack build crates/wasm --target web --out-dir ../../web/public/wasm --out-name round_eliminator_3_wasm --release
cd web && npm ci && npm run build && npm run test:wasm
cd .. && cargo run -p round-eliminator-3-server
```

Open `http://127.0.0.1:8080`. The selector beside Start chooses Server or
WebAssembly; both send the same JSON request to `execute_json` in the core
crate. For frontend development, run `npm run dev` in `web` while the Rust
server runs; Vite proxies `/api` to it. The WebAssembly build is needed before
selecting WebAssembly.

`cargo test --workspace` also regenerates the Rust-defined TypeScript types in
`web/src/generated`. Commit changes to those files alongside protocol changes.
