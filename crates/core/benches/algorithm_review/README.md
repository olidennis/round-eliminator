# Seven requested algorithm experiments

2026-10-14. The numbers below compare the current rewrite immediately before
this experiment batch against selected prototypes. **They do not compare against
the original round-eliminator repository.** Requested items refer to the user's
latest list: 1, 2, 3, 4, 5, 6 and 10.

## Method

Linux/aarch64, Rust 1.89 release, one Rayon worker, configured pinned mimalloc v2
(option 26 = 0). Builds/tests finish before measurements. Most rows are medians
of three interleaved before/after samples, each at least 60 ms; long operations
complete their entire call. Parsing, RE fixture preparation and independent
witness checks occur outside timings. Before/after results must agree; where
SAT can choose a different strategy, decision/palette results agree and each
strategy is verified independently. RSS probes run each variant in a separate
process using /usr/bin/time; those are single-process samples, not medians.

prepare.py copies the core to temporary packages and overlays frozen baseline
kernels. Benchmark exports exist only in those copies. --only selects a kernel;
zero-round alternatives compare against explicit port SAT in the same selected
core to isolate shared traversal effects. No experiment flags/dependencies are
added to production. The rejected membership and two symmetry implementations
are retained here for reproduction. The measured recursive degree-two pilot is
also retained; production uses an explicit stack and its final timings below.

## Decisions

| User item | Decision | Evidence |
| --- | --- | --- |
| 1. Enumeration memory | Integrated | 2.55–2.82× faster on unique-distribution fixtures; overlapping fixture approximately unchanged. |
| 2. Diagram contexts | Integrated, with a cheap degree-two eager path | Shared lazy contexts and shared profile-pair coverage give up to 14.18×; sparse ring remains about 5% slower (a few microseconds). |
| 3. Membership zero-round SAT | Not the default | Improvements up to 1.89×, regressions up to 1.86×; no broad replacement justified. |
| 4. Degree-two support search | Integrated for plain problems with passive degrees at most two | Exact compatible-support search avoids port SAT on the eligible fixtures. Explicit stack preserves deep configurations and cancellation. Inputs retain general SAT. |
| 5. Distance-two symmetry | Not integrated | Both full simultaneous-swap lex leaders and cheaper row-union profile ordering regress all four measured fixtures. |
| 6. Basic FP lattice tables | Integrated hybrid | Keep full tables through 128 values, select the table algebra once; use direct operations above that. Large preparation/memory improve, individual direct operations cost more. |
| 10. Cancelled search resumption | Integrated | Persist the proved interval and witness per coloring kind. Regression tests resume at the next unknown palette, never infer UNSAT from cancellation. |

## Timings

| Experiment / fixture | Before (ms) | Selected/prototype (ms) | Speedup |
| --- | ---: | ---: | ---: |
| 1-enumeration: single | 54.0236 | 19.1443 | 2.82× |
| 1-enumeration: disjoint | 42.3180 | 16.5968 | 2.55× |
| 1-enumeration: overlapping | 0.2362 | 0.2422 | 0.98× |
| 2-diagram: all64 | 0.1345 | 0.1151 | 1.17× |
| 2-diagram: all256 | 2.1123 | 1.9104 | 1.11× |
| 2-diagram: required64 | 1.4426 | 0.1017 | 14.18× |
| 2-diagram: sparse64 | 0.0674 | 0.0708 | 0.95× |
| degree_two: plain balanced 8 SAT | 0.0708 | 0.0004 | 160.54× |
| degree_two: plain balanced 8 UNSAT | 0.0699 | 0.0002 | 307.72× |
| degree_two: plain overlapping 16 SAT | 0.1578 | 0.0007 | 223.49× |
| degree_two: plain overlapping 16 UNSAT | 0.1563 | 0.0003 | 577.84× |
| degree_two: plain large capacity 64 SAT | 0.9454 | 0.0007 | 1393.84× |
| degree_two: plain dense 12 SAT | 0.2935 | 0.0018 | 166.46× |
| degree_two: plain dense 24 SAT | 1.0173 | 0.0027 | 373.52× |
| degree_two: plain dense 48 SAT | 3.8829 | 0.0046 | 835.81× |
| degree_two: plain union SAT | 0.1274 | 0.0007 | 185.76× |
| 6-basic-full: given-basic | 22585.3954 | 22541.9886 | 1.00× |
| 6-basic-construction: 8 independent labels | 11.0127 | 2.5344 | 4.35× |
| 6-basic-construction: 12 independent labels | 3064.2845 | 203.1432 | 15.08× |
| 10-resume: incidence RE(BMM) | 14.7621 | 14.5166 | 1.02× |
| 10-resume: graph edges | 0.2465 | 0.2291 | 1.08× |
| 10-resume: vertex | 0.0955 | 0.0933 | 1.02× |
| 10-resume: distance two | 0.2352 | 0.2375 | 0.99× |

The full given Basic FP case includes simplification and is **serial**: roughly
22.6 seconds both ways. The 8/12-independent-label rows measure complete Basic
construction (both closures and conversion), excluding discard-useless
simplification, with identical constructed problems. They improve 4.35×/15.08×.
Do not present these as measured whole-button gains on arbitrary large lattices.

## Memory and lattice operations

| Probe | Before peak RSS (MiB) | After peak RSS (MiB) |
| --- | ---: | ---: |
| single | 36.84 | 6.57 |
| disjoint | 26.84 | 6.58 |
| lattice4096 | 151.03 | 9.19 |

At 4,096 values, full lattice preparation falls from about 3 seconds to under
1 ms. The direct-operation path retains the full lattice and preserves both
algebras, including the empty value; it removes quadratic tables, not exponential
lattice enumeration. Direct operations are slower individually: see lattice.jsonl
for preparation and 10,000-operation samples. TableOperations selects the small
hot path once before maximization, avoiding a representation branch per call.

## Correctness arguments

- Fixed singleton parts contribute constant counts. With at most one variable
  part, or mutually disjoint variable domains, the concrete multiset uniquely
  determines every part distribution. Otherwise retain the old visited set and
  its first-origin provenance. Independent small expansions verify visit sets
  and counts, including overlaps and repetition.
- Equal occurrence profiles remove the same part positions, hence have the same
  residual language. Cache residual preparation and exact profile-pair coverage;
  never identify arbitrary semantic-equivalent constraints. Mostly distinct
  degree-two profiles retain direct eager vector comparisons.
- For plain degree-two problems, every emitted label needs a permitted self-pair,
  and every pair of emitted labels must be compatible. A valid support can satisfy
  an active condensed row by choosing one supported label per part. Choices across
  all allowed active degrees share the same support. Degree zero/one passive
  obligations are also checked; inputs use the existing SAT path. Deep-stack and
  cancellation tests and comparison with port SAT guard the implementation.
- Membership SAT uses existential active matching and universal passive checking
  on one emission table per input. A valid old strategy supplies its emission
  union; a valid table supplies active assignments. Its greedy matching-based
  final reconstruction and changed solver behavior make it slower on some cases.
- Full symmetry leaders compare a matrix against simultaneous row/column swaps;
  the orbit's lexicographic minimum survives. Row-union profiles are invariant
  under neighbor-column permutations, so sorting them is also safe. Per-palette
  gates preserve growth/shrinkage. Exhaustive small-matrix tests plus independent
  actual witnesses validated the prototypes before rejecting them for speed.
- Bounds merge monotonically, retaining the strongest lower witness and tightest
  upper bound. The interrupted query is never a proof. Results remain scoped to
  the same immutable problem and coloring kind. Final complete certificates
  supersede partial intervals in the client cache.

## Resumption limits

The measured first-palette interruptions save decisions but mostly change elapsed
time by only 0–8%; the expensive next query often dominates. Distance-two order
preflight is still repeated when no upper bound proves finiteness. This change
resumes numerical proofs; it does not retain SAT clauses or backend sessions.
All kinds can reuse saved lower/upper bounds. Saved UI bounds migrate to shared
state; current files store proofs only there, without duplicating them in controls.

## Verification

181 Rust workspace tests, frontend build/typecheck, session migration and client
concurrency/cancellation checks passed. Rebuilt server/wasm agree on all four
input variants, 30 cache fixtures, 39 coloring and 151 diagram fixtures. Browser
checks resume imported partial proofs on both backends and exercise RE/cache/UI.

## Reproduce

```sh
python3 crates/core/benches/algorithm_review/prepare.py /tmp/re3-seven --build
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/algorithm-review enumerate
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/algorithm-review diagram
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/algorithm-review zero degree_two
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/algorithm-review lattice
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/algorithm-review basic
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/algorithm-review basic_construct 12
RAYON_NUM_THREADS=1 /tmp/re3-overlap-target/release/algorithm-review resume
```

Use --only to isolate individual kernels. Add --symmetry tensor or --symmetry
profiles to reproduce a rejected symmetry implementation. The two rejected symmetry sources are
retained as distance_two_tensor.rs and distance_two_profiles.rs; baseline symmetry
is restored in production. Raw JSON files retain all samples, including the
initial slower branching-lattice prototype. Build first, run one measurement
process at a time. Future runs use current surrounding code; historical numbers
above describe this recorded batch.
