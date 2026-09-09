# Local CPU-time statistics fix

This is `cpu-time` 1.0.0 from crates.io (upstream:
https://github.com/tailhook/cpu-time), retaining its MIT/Apache-2.0 licenses.
The Rust sources, package manifest, README and licenses were copied from that
release. No system-wide Cargo cache modification is needed.

The local change makes `ProcessTime::duration_since` and
`ThreadTime::duration_since` use saturating subtraction on Unix and Windows.
If a CPU-clock reading regresses, the elapsed CPU time is reported as zero
instead of panicking with `overflow when subtracting durations`. Nonnegative
deltas and OS clock-error handling are unchanged. Platform modules also include
`src/regression_tests.rs`, testing backwards, equal and increasing timestamps.

RustSAT/MiniSat 0.3.1 records `start.elapsed()` immediately after solving, before
updating its SAT/UNSAT state. A panic there loses a completed solve's result.
This fix only changes CPU-time statistics: it does not catch and reinterpret a
failed solve, replace MiniSat, disable parallelism, or change search deadlines.
Round Eliminator's reversible-edge deadlines use `std::time::Instant`.

The library (including native examples/tests), server and CLI root manifests
each override crates.io's `cpu-time` with this directory. Cargo does not inherit
a dependency's `[patch.crates-io]` section, so a separate application using
Round Eliminator as a dependency must add the same override at its own root,
with a path pointing to this directory. No WASM changes are required.

Verify the override from the native build directory with:

```sh
cargo tree -i cpu-time
```

It must show this local `vendor/cpu-time` path. Test the clock fix from the
repository root with:

```sh
cargo test --manifest-path vendor/cpu-time/Cargo.toml --lib
```
