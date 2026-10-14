# Algorithm benchmarks

## Coloring solvability

The [coloring investigation](coloring/README.md) reproduces the native incidence
edge coloring slowdown after one RE on degree-ten BMM. Temporary prototypes
test color symmetry, eager passive degree-two clauses and output-domain grouping
with exact provenance. It compares all four coloring kinds, including original
inputs, on native and wasm. Selected prototypes are verified but **not integrated**.

## RE and maximization against the old version

The [RE comparison](re_comparison/README.md) measures ten elimination steps on
degree-10 bipartite maximal matching and two steps on the second old `test_all()`
fixture. It separates raw RE, the old button workflow, and maximization on identical
inputs. Both libraries are built in one isolated native release benchmark, with
the old implementation excluded from production dependencies. Per-step timings,
input/output sizes, equivalence checks, and repeatable commands are recorded there.
The follow-up [profiling investigation](re_comparison/investigation/README.md)
isolates pruning, matching allocation, dominance scans, and scheduling costs;
controlled trials run only in temporary library copies; selected optimizations
are now integrated into production. The [aggregate-work comparison](re_comparison/investigation/README.md#aggregate-work-versus-fully-serial-execution)
measures summed CPU time and elapsed time at 1, 4 and 10 workers on identical inputs.
The [Basic fixed-point/allocator report](re_comparison/investigation/allocator-comparison.md)
also tests the custom reversed algebra and empty lattice values, and compares
system, jemalloc and configured mimalloc allocation without changing production.
The [current performance investigation](re_comparison/investigation/current-performance.md)
identifies late duplicate checks and matrix rejection in Basic, and per-pair
task/channel overhead on the large fixture. The selected improvements are now
integrated; it records elapsed and aggregate CPU time, with historical kernels
retained as benchmark fixtures.
The [adaptive generation-batch follow-up](re_comparison/investigation/adaptive-batches.md)
benchmarks fixed sizes from 256 to 4,096 and a timing-based policy. The recommended
prototype reduces Basic CPU by 15% and elapsed by 7%, with matching and the large
fixture effectively unchanged. This policy is now integrated into native
untraced generation; serial, wasm and observed derivations keep fixed batches.

The [live-pruning comparison](re_comparison/investigation/live-pruning.md)
combines immediately shared pruning decisions, packed small alphabets, and
adaptive pruning concurrency. It compares the large fixture, Basic and matching,
including uninstrumented serial/parallel aggregate CPU measurements. These
improvements are now integrated; historical snapshot variants remain reproducible.

The [review experiment follow-up](re_comparison/investigation/review-improvements.md)
compares early pair rejection, prepared alignment traversal, component balance,
compatibility caching, generated-history sharing and 128-bit packing. It retains
controlled prototypes and confirms the selected changes in uninstrumented native
executables, with complete-output comparisons, total CPU and per-process peak RSS.

The [four final categories](re_comparison/investigation/final-four.md) include
flow-network retention, density-dependent multiword masks, bounded overlap,
exact dictionary/varint history and conditional generation grains. Every category
was tested; only measured beneficial choices were integrated. The native server
now shares the benchmark's pinned mimalloc-v2 configuration.
The [CPU budget follow-up](re_comparison/investigation/packed-cpu.md) finds a
packed-set/coarse-generation/serial-pruning prototype for the large fixture:
1.91 s elapsed, 4.29 s aggregate CPU, and 29.2% overhead over its own serial path.
This performance prototype has not been integrated.

## Passive diagrams

```sh
RAYON_NUM_THREADS=1 cargo bench -p round-eliminator-3 --bench diagram
RAYON_NUM_THREADS=4 cargo bench -p round-eliminator-3 --bench diagram
```

The fixtures are genuinely maximized: a complete bipartite compatibility relation,
and a degree-13 constraint requiring at least one label from the first half of
the alphabet. Adding an outside label to the required part permits an invalid
multiset, so the latter condensed configuration is maximal. Both methods are
checked for identical results before timing. No maximization algorithm is used.
The recorded times cover the former API returning all three views, context
preparation, and SCC reduction. The current benchmark requests only the output
diagram, matching the API's one-diagram-per-request behavior; new measurements
therefore cover less orchestration work than this historical table.

Measured 2026-10-12 on ARM64 Linux, Rust 1.89.0, Rayon 1.12.0, native release.
Each value is the mean of 100 requests after warm-up, in milliseconds.
Raw measurements are in [diagram-results.csv](diagram-results.csv).

| Fixture | Maximized, pool 1 | Maximized, pool 4 | Exact, pool 1 | Exact, pool 4 |
| --- | ---: | ---: | ---: | ---: |
| 64 labels, passive degree 2 | 0.280 | 0.274 | 0.317 | 0.313 |
| 256 labels, passive degree 2 | 4.658 | 4.631 | 5.841 | 5.707 |
| 64 labels, passive degree 13 | 0.320 | 0.310 | 2.214 | 1.083 |

Earlier measurements that parallelized cheap comparisons were slower: at 64
labels, roughly 0.58 ms versus 0.27 ms for the maximized method. The selected
scheduling policy keeps these cases serial even with a larger pool. General
exact coverage uses workers; large occurrence profiles can also use them.
Pool size therefore does not mean every request uses that many threads. These
fixtures demonstrate useful parallelism and the maximization shortcut, not a
universal speed ranking. Wasm retains the serial algorithm in a request worker.

## Configuration membership

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

### Measurements and choice

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

### Cancellation

Cancellation is checked during network construction and before and after each
flow call. Petgraph provides no callback inside a flow call. A native cancellation
therefore waits for that call to finish; the largest measured call was a few
milliseconds including construction, but larger networks can take longer.
Wasm worker termination can interrupt the call itself.

## Zero-round cardinality encodings

Run the complete-request benchmarks from the repository root:

```sh
cargo bench -p round-eliminator-3 --bench zero_round
node crates/core/benches/zero_round_wasm.mjs
```

Build release wasm first using the command in the main README. Both runners use
`support/zero_round_cases.json`: solvable and unsolvable examples covering all
four entry forms, nonregular degrees, overlapping/dense parts, larger capacities,
and passive counterexample refinement. The cardinality measurements below used
the original 19 fixtures; ten passive-input fixtures were subsequently added.
Notation is parsed once before timing.
Each measured request includes model deserialization, formula construction, SAT
solving, passive verification/refinement, and response serialization. It excludes
HTTP, UI rendering, and notation parsing. Native checks independently validate
solvable witnesses outside timing; both runners assert the expected decision.

The wasm runner accepts multiple package directories, alternating sample order:

```sh
node crates/core/benches/zero_round_wasm.mjs /tmp/wasm-before /tmp/wasm-after
```

Save each version's wasm package before building the next. Native revisions can
likewise be compared by saving the compiled benchmark executable from
`target/release/deps/zero_round-*` before rebuilding, then alternating runs of the
saved executables. Keep other builds/benchmarks idle while measuring.

Measured on 2026-10-11 in the ARM64 Linux development container, Rust 1.89.0,
RustSAT 0.7.5, CaDiCaL 2.2.1 native, BatSat wasm, Node v25.9.0, release builds
(with wasm-opt for wasm). The reference is the former hand-written sequential
counter. The library version uses Ladder for capacity one and Totalizer for
larger bounds; zero capacity emits negative unit clauses and a capacity at least
the number of literals requires no clauses. Both versions check cancellation.
The rest of the problem model, matching algorithm, backend and search are the
same. No solver tuning was performed.

Times are milliseconds per complete request. Each run reports the median of
five samples, with at least 100 ms per sample. Native columns are medians of two
run medians per implementation, run in library/custom/custom/library order after
initial exploratory measurements. Wasm columns are medians of five alternating
samples after warm-up.

| Complete check | Native custom | Native library | Wasm custom | Wasm library |
| --- | ---: | ---: | ---: | ---: |
| plain small SAT | 0.0357 | 0.0345 | 0.0139 | 0.0123 |
| plain small UNSAT | 0.0364 | 0.0351 | 0.0143 | 0.0125 |
| plain balanced 8 SAT | 0.0783 | 0.0758 | 0.0481 | 0.0473 |
| plain balanced 8 UNSAT | 0.0785 | 0.0756 | 0.0416 | 0.0418 |
| plain overlapping 16 SAT | 0.1958 | 0.1672 | 0.1397 | 0.1128 |
| plain overlapping 16 UNSAT | 0.1947 | 0.1672 | 0.1149 | 0.1047 |
| plain large capacity 64 SAT | 1.5959 | 1.0137 | 1.2600 | 0.7639 |
| plain dense 12 SAT | 0.3352 | 0.3298 | 0.7997 | 0.6459 |
| plain dense 24 SAT | 1.0473 | 1.1163 | 3.7040 | 3.5178 |
| plain dense 48 SAT | 3.9330 | 4.2210 | 25.7139 | 16.1835 |
| plain union SAT | 0.1487 | 0.1391 | 0.0919 | 0.0856 |
| plain refinement degree 8 SAT | 0.0445 | 0.0437 | 0.0241 | 0.0222 |
| plain refinement degree 8 UNSAT | 0.0373 | 0.0365 | 0.0165 | 0.0150 |
| independent 6 SAT | 0.3056 | 0.2668 | 0.2715 | 0.2082 |
| independent 10 SAT | 0.8202 | 0.7544 | 1.1859 | 0.6259 |
| mapped 6 SAT | 1.1487 | 1.0074 | 5.3740 | 0.8719 |
| mapped 6 UNSAT | 0.0731 | 0.0802 | 0.0644 | 0.0657 |
| paired 6 SAT | 0.1403 | 0.1283 | 0.0919 | 0.0825 |
| nonregular SAT | 0.0516 | 0.0494 | 0.0261 | 0.0238 |

Choose the library implementation. Native results are mostly comparable or
faster, with about 7% slowdowns on dense degree-24/48 cases and about 10% on the
small unsolvable mapped example (an absolute difference of roughly 0.007 ms).
Wasm is comparable or faster, including approximately 6.2x faster on the mapped
solvable example, 1.9x on independent degree 10, and 1.6x on dense degree 48.
Different CNFs can change the solver's search and witnesses; these synthetic
fixtures do not establish a universal ranking. Add actual difficult problems to
the same fixture file when revisiting the choice.

The library clause collector checks cancellation before sending every generated
clause. Its trait exposes only `OutOfMemory`; a private adapter retains the
actual cancellation or solver error and restores it on return, so cancellation
is never reported as an allocation failure or unsolvability. Setup/allocation
before clause emission has no library cancellation hook. Wasm worker termination
also interrupts that work.

## Passive-input enumeration and maximized constraints

Run the comparison from the repository root:

```sh
python3 crates/core/benches/passive_inputs/run.py --output /tmp/passive-input-results.csv
```

Use `--native-only` to omit wasm, or `--toolchain VERSION` to change the Rust
compiler (default 1.89.0). Python and Cargo are required, plus Node and wasm-pack
for wasm. `--wasm-package PATH` uses an already built current release package.
Keep other builds and CPU-heavy work idle while measuring. The runner builds the
current library once, then compares general and maximized requests through the
same native executable and wasm package. It does not modify source files.

General fixtures run only in general mode; hand-written maximized fixtures run in
both modes. Native runners independently check witnesses and validate the
maximized fixtures outside timing. To compare modes directly in an existing wasm
package:

```sh
node crates/core/benches/zero_round_wasm.mjs --compare-modes \
  --cases crates/core/benches/support/zero_round_maximized_cases.json
```

### Initial prototype experiment

Before adoption, four implementations were compared:


- **Expanded:** the current passive-input multiset enumeration and exact output
  checker; no maximization requirement.
- **Condensed:** retain passive input parts, union their compatible candidate
  outputs, then enumerate concrete output multisets lazily. Stop at the first
  violation. Preserve emission origins when producing a counterexample clause.
  Equal temporary output parts with equal origins are combined to avoid redundant
  traversal; this does not normalize the model. No maximization requirement.
- **Maximized refinement:** transform candidate emissions to condensed output
  parts and test containment in one passive configuration. Match parts using
  subset edges and counted capacities with Dinic (simple cases use set inclusion).
  A failed containment check blocks the selected emissions without enumerating
  a concrete bad output. This rejection is sound only for a maximized constraint.
- **Maximized direct:** encode that containment and capacity matching in SAT
  upfront, with guards for input families that have no compatible emissions.
  If all potential emissions already fit a target, omit that family's clauses.
  No passive refinement during search. Requires a maximized constraint.

Expanded, Condensed, and Maximized refinement retain the existing eager clauses
for passive degrees at most two. Their refinement comparison concerns higher
passive degrees. Maximized direct encodes containment at every passive degree.

The initial general suite contained 29 fixtures: the original 19 plus ten cases with
broad/overlapping input parts, fixed outputs, distinct output alphabets, and
repeated parts. Only Expanded and Condensed ran on it. The maximized suite had
23 hand-written fixtures and ran all four implementations. It includes:

- Universal full-alphabet parts; monochromatic alternatives `X^d` / `Y^d`.
- Exact counts `X^3 Y^3`, and at most one X, `XY Y^5`.
- Three-label configurations excluding monochromatic outputs. At degree d=3 or
  d=6, the complete maximal list is `AB AC BC ABC^(d-3)`, `A BC ABC^(d-2)`,
  `B AC ABC^(d-2)`, `C AB ABC^(d-2)`.
- Nonregular degrees including zero, and canonicalized paired entry.

`support/maximized.rs` checks the supplied constraints outside timing. For two
labels it tests every interval of possible label counts. For the non-universal
three-label fixtures it enumerates all condensed families and compares concrete
semantic coverage with containment using an independent exhaustive assignment
oracle. Larger alphabets in these fixtures have a universal full-alphabet part.
These checks assert completeness of the hand-written lists; they do not compute
or implement maximization.

Both suites assert expected SAT/UNSAT decisions. Native runs also check witnesses
using the general checker, even for maximized variants. Before the initial measurement, the
script ran all nine condensed-prototype zero-round tests, including exhaustive
comparison of passive checks and counterexample origins over small emission
subsets. The ordinary union-of-configurations tests remain valid without any
maximization assumption.

Measurements time complete structured requests after parsing notation once,
using CaDiCaL 2.2.1 native and BatSat wasm. Each sample lasts at least 100 ms.
Native results are medians of two run medians (five samples per run), with the
second run reversing implementation order. Wasm results are medians of five
samples alternating implementation order after warm-up. Builds and measurements
run sequentially. HTTP, worker startup and rendering are excluded.

### Initial results and selection

Measured on 2026-10-11 in the ARM64 Linux development container, Rust 1.89.0,
RustSAT 0.7.5, release builds, wasm-opt, Node v25.9.0. Full results for every
fixture and implementation are in [passive_inputs/results.csv](passive_inputs/results.csv).
These are synthetic workloads; timings do not establish a universal ranking.
All decisions agreed with the expected results, all native witnesses passed the
general verifier, and all hand-written maximized constraints passed the checks.

Times below are milliseconds per complete request. The first table uses the
**general** suite, which has no maximization assumption.

| Case | Native expanded | Native condensed | Wasm expanded | Wasm condensed |
| --- | ---: | ---: | ---: | ---: |
| plain small SAT | 0.0372 | 0.0343 | 0.0128 | 0.0127 |
| plain dense 48 SAT | 4.2493 | 4.4133 | 16.5547 | 17.1801 |
| broad inputs 4 labels degree 8 SAT | 0.1400 | 0.0421 | 0.1178 | 0.0206 |
| broad inputs 8 labels degree 10 SAT | 16.4433 | 0.0570 | 21.8861 | 0.0342 |
| broad inputs 8 labels degree 12 SAT | 47.0194 | 0.0579 | 53.9350 | 0.0347 |
| broad inputs fixed outputs degree 10 SAT | 16.7364 | 0.0433 | 19.6752 | 0.0232 |
| distinct input outputs 8 labels degree 6 SAT | 1.2561 | 0.4555 | 1.3659 | 0.4904 |
| three forced outputs repeated parts degree 10 SAT | 0.4421 | 0.0854 | 0.4741 | 0.0572 |
| overlapping passive inputs UNSAT | 0.0401 | 0.0355 | 0.0184 | 0.0148 |

The original 19 fixtures were comparable: native changes ranged from about 8%
faster to 4% slower; wasm changes were within about 4%. Broad input families
benefited much more, particularly when many inputs share few outputs. Degree 12
with eight input labels improved approximately 812x native and 1553x wasm.
Fixed outputs also benefit: expansion previously enumerated input multisets even
though the mapping was already determined.

The next table uses only **maximized** passive constraints. It compares the
condensed general checker with the two assumption-based implementations.

| Case | Native condensed | Native refinement | Native direct | Wasm condensed | Wasm refinement | Wasm direct |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| distinct input outputs 8 labels degree 6 SAT | 0.4558 | 0.0434 | 0.0434 | 0.4882 | 0.0219 | 0.0216 |
| three forced outputs repeated parts degree 10 SAT | 0.0867 | 0.0751 | 0.0773 | 0.0557 | 0.0467 | 0.0463 |
| maximized monochrome plain SAT | 0.0331 | 0.0336 | 0.0432 | 0.0107 | 0.0106 | 0.0171 |
| maximized at most one X UNSAT | 0.0304 | 0.0315 | 0.0466 | 0.0099 | 0.0104 | 0.0225 |
| maximized exact counts UNSAT | 0.0299 | 0.0301 | 0.0472 | 0.0095 | 0.0094 | 0.0249 |
| maximized three labels degree 6 SAT | 0.0345 | 0.0346 | 0.0358 | 0.0157 | 0.0158 | 0.0156 |
| maximized three labels degree 6 UNSAT | 0.0326 | 0.0331 | 0.3258 | 0.0140 | 0.0138 | 0.0978 |

Recommend **Condensed** as the general replacement for passive-input expansion,
and **Maximized refinement** when maximization is guaranteed. On the distinct
output-alphabet example, containment refinement is about 10.5x faster than the
condensed checker natively and 22.3x faster in wasm. It works on sets and
multiplicities during refinement, avoiding concrete output enumeration there.

Direct containment encoding was comparable or slightly faster on several small
wasm solvable cases. Its formula construction cost hurt other cases: roughly
9.9x slower native and 7.1x slower wasm on the three-label degree-six unsolvable
fixture. These examples do not justify choosing it as the default. Refinement
with a whole-emission blocking clause can require more candidates on harder
instances; these measurements do not settle that tradeoff for every problem.

Condensed inputs alone do not remove output combinatorics. The prototype keeps
visited output multisets to suppress overlap duplicates, which can consume memory
on broad valid output families; peak memory was not measured. A containment proof
can safely accept a family even without maximization. A failed containment test
on an unmaximized constraint still needs the exact general check: separate listed
configurations may cover the family jointly. The adopted modes now share that safe acceptance shortcut. It was not part of
the initial Condensed prototype measured above.

Following the user's choice, the two selected modes are now implemented in
production with shared containment preparation. The expanded-input and direct
SAT prototypes and their temporary patching runner have been retired. The CSV
above preserves their historical measurements; the current runner measures the
two adopted modes and does not reproduce the retired prototypes.

### Adopted implementation

The adopted general mode first tries a containment proof, then enumerates outputs
lazily only if that proof fails. The maximized mode uses the same proof and can
reject immediately on failure. Both share preparation, eager clauses for passive
degrees at most two, the SAT search, cancellation, and independent witness checking.
The API mode is explicit; the GUI uses general mode until cached maxima are
integrated into solvability checks.

The current measurements are in
[passive_inputs/adopted-results.csv](passive_inputs/adopted-results.csv), using the
same environment, fixtures and sampling method as the initial experiment.
All expected decisions, native witnesses and maximization checks passed.
The frontend build and wasm/server parity checks also passed for both modes.

Times are milliseconds per complete request on the maximized fixture suite:

| Case | Native general | Native maximized | Wasm general | Wasm maximized |
| --- | ---: | ---: | ---: | ---: |
| broad inputs 8 labels degree 12 SAT | 0.0551 | 0.0557 | 0.0324 | 0.0322 |
| distinct input outputs 8 labels degree 6 SAT | 0.0423 | 0.0422 | 0.0222 | 0.0223 |
| three forced outputs repeated parts degree 10 SAT | 0.0741 | 0.0740 | 0.0465 | 0.0465 |
| maximized monochrome plain UNSAT | 0.0358 | 0.0353 | 0.0137 | 0.0132 |
| maximized three labels degree 6 UNSAT | 0.0325 | 0.0327 | 0.0141 | 0.0140 |

Both modes now benefit from containment proofs on valid families. For example,
the distinct-output general fixture improved from 0.456 to 0.043 ms native and
0.490 to 0.023 ms wasm compared with the initial condensed-only prototype.
The adopted general and maximized modes were comparable on the maximized suite;
the difference is the work after failed containment. General mode must find a
concrete violation; maximized mode can block the whole selected family directly.
On the original 19 general fixtures, times stayed within about 6% of the initial
condensed prototype, with comparisons taken in separate runs. These small
changes do not indicate a universal ranking. Harder invalid families may show
larger differences; maximize only when its total cost is justified or when a
maximized constraint is already needed by another algorithm.
