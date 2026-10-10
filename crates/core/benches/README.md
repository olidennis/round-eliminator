# Configuration membership benchmarks

Run from the repository root:

```sh
cargo bench -p round-eliminator-3 --bench configuration_matching
node crates/core/benches/wasm.mjs
```

The wasm command requires Node and wasm-pack. It creates and removes a temporary
adapter crate, using the same Rust fixtures as the native benchmark. Set
`RUSTUP_TOOLCHAIN` if a specific installed toolchain should build it.

Both commands emit CSV. `original` is the previous occurrence matcher, retained
only as a benchmark reference. `selected` calls the current public membership
function. `dinic` and `edmonds-karp` compare the two petgraph flow algorithms
without the selected implementation's elementary shortcuts and capacity rejection.

Timings include fresh allocation, network construction, cancellation checks, and
matching. Constructing the constraint and concrete multiset is excluded. The old
matcher receives its occurrence vector already expanded, so its reported time
also excludes conversion from counts. Wasm batches stay inside Rust to exclude
JS call overhead. Reported values are medians of three samples; these are
synthetic cases, not a general performance ranking.

## Measurements and choice

Measured on 2026-10-10 in the ARM64 development container, with Rust 1.89.0,
release builds, petgraph 0.8.3, and Node v25.9.0 for wasm. Times below are
microseconds per membership check, rounded.

| Case | Native original | Native selected | Wasm original | Wasm selected |
| --- | ---: | ---: | ---: | ---: |
| Degree 2, reassignment | 0.155 | 0.706 | 0.100 | 0.753 |
| Degree 3, overlapping parts | 0.205 | 0.932 | 0.152 | 0.952 |
| Degree 10, overlapping parts | 0.411 | 0.924 | 0.299 | 1.047 |
| Degree 50, overlapping parts | 1.781 | 0.915 | 1.661 | 1.011 |
| Degree 1,000, overlapping parts | 110.454 | 0.929 | 172.880 | 1.013 |
| Degree 10,000, overlapping parts | 9,082.538 | 0.935 | 15,523.677 | 1.036 |
| Degree 1,000, one broad part | 20.761 | 0.067 | 17.855 | 0.033 |
| 16 distinct labels, dense | 1.853 | 9.287 | 2.165 | 8.928 |
| 64 distinct labels, dense | 44.220 | 122.852 | 71.339 | 126.632 |
| 256 distinct labels, dense | 1,988.159 | 2,121.598 | 3,906.045 | 2,218.489 |
| Degree 10, retry a second configuration | 0.641 | 1.052 | 0.449 | 1.028 |

The repeated-count cases have a fixed number of distinct labels and parts while
their exponents grow. Flow works on those counts directly; the old matcher works
on every occurrence. Dense cases use distinct labels and many overlapping parts.

Dinic was selected. The custom matcher is clearly faster for small individual
checks and moderately dense cases. Dinic wins with large repeated counts and is
comparable or faster on the largest dense case. Edmonds–Karp is faster on the
small flow networks, but much slower on dense networks: about 72.6 ms native and
77.3 ms wasm at 256 distinct labels, versus about 2 ms for Dinic.

Complete zero-round checks were also compared in wasm against a snapshot taken
before the counted representation and flow changes. Problems were parsed once;
only the structured analysis request was timed. Five samples were alternated
between implementations after warm-up. Times are milliseconds per request.

| Complete check | Previous | Selected |
| --- | ---: | ---: |
| Plain, degree 3, solvable | 0.0136 | 0.0133 |
| Plain, degree 3, unsolvable | 0.0145 | 0.0157 |
| Plain, degree 8, union of exact multisets | 0.0240 | 0.0245 |
| Plain, degree 12, union of exact multisets | 0.0372 | 0.0326 |
| Independent inputs, degree 6 | 0.2626 | 0.2756 |
| Mapped inputs, degree 6 | 0.0447 | 0.0451 |
| Paired inputs, degree 6 | 0.0915 | 0.0937 |

The plain examples have active constraint `A B`. Their passive constraints are
`AB^3`, `AB^2 A`, or the union of every `A^k B^(d-k)` at the stated degree.
The independent example has input `ab^6` / `ab^3`, output `A^3 B^3` / every
degree-three multiset. The mapped example has the same input, output `AB^6` /
`AB^3`, and mapping `a -> A`, `b -> B`. The paired example allows all four
input/output pairs repeated six times actively and three times passively.

These complete runtimes are comparable, which favors the library under the
user's preference. The choice also avoids maintaining a custom matching
algorithm and makes large multiplicities practical. It does not promise a
speedup on every future workload; rerun and extend these fixtures as needed.

## Cancellation

Cancellation is checked during network construction and before and after each
flow call. Petgraph provides no callback inside a flow call. A native cancellation
therefore waits for that call to finish; the largest measured call was a few
milliseconds including construction, but larger networks can take longer.
Wasm worker termination can interrupt the call itself.
