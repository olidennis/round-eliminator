# Round Eliminator 3 project brief

This branch is a ground-up rewrite. The source worktree is `/workspace/round-eliminator`
on `llm-enhanced`; this worktree is `/workspace/round-eliminator-3` on `re3`.
Keep both worktrees. Read only `round-eliminator-lib`, `round-eliminator-server`,
`round-eliminator-wasm`, and the GUI files needed to understand them in the old tree.

## Goal and scope

- Build readable, fast, idiomatic Rust, with coherent modules, tests, and comments.
- Keep abstractions few and justified, but leave room for all old GUI features and future features.
- Eventually handle non-regular active/passive graphs and problems with input labels.
- Input-independent output validity is a pair of ordinary input/output problems.
- Input-dependent validity may be expressed either as allowed multisets of
  `(input label, output label)` pairs on each side, or as an input problem plus
  output constraints and a map `input label -> allowed output labels`.
  The latter is described in Section 6.1, Definition 6.1 of
  https://arxiv.org/pdf/2510.17639 (printed page 38).
- Consider GPU execution only when a concrete algorithm and benchmark justify it.
- The GUI should be able to use both wasm and a server through one coherent API.
  Avoid adding an independent manual request mapping in both JavaScript and Rust.
- Framework choices, including Vue, are open.

## Workflow

`FEATURES.md` inventories existing GUI behavior as unimplemented work. The first
planned implementation is the equivalent of Start: enter a problem, send it to
Rust, and display the returned problem. Before implementing that or any later
feature, discuss meaningful implementation alternatives and their tradeoffs
with the user, wait for their choice, then implement the selected feature with
tests and comments. Do not import old algorithms wholesale or implement further
features ahead of that choice. Treat the source tree as reference, not as code
to preserve for its own sake.
