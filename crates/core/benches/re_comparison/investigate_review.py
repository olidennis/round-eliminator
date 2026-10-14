#!/usr/bin/env python3
"""Build controlled review experiments from the frozen live-pruning baseline.

No profile flags reach production. Both paths pay the same flag lookup, once
per parent pair/matching query, so tiny differences still need confirmation in
an uninstrumented executable. Run after building, without other CPU work.
"""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
from investigate import patch_source

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
BASE = HERE / "investigation/baseline/2026-10-13/review"


def replace(path, old, new):
    text = patch_source(path.read_text())
    assert text.count(old) == 1, (path, old, text.count(old))
    path.write_text(text.replace(old, new))


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=True)
    core = directory / "core"
    if core.exists():
        shutil.rmtree(core)
    shutil.copytree(ROOT / "crates/core/src", core / "src")
    for path in BASE.rglob("*.rs"):
        shutil.copy(path, core / "src" / path.relative_to(BASE))
    # Later production added compressed history/wider adapters. Restore the
    # wrapper from the reviewed generation too; its packed module remains the
    # frozen u64 adapter and these experiments add their own u128 prototype.
    before = HERE / "investigation/baseline/2026-10-13/final-four/algorithms/maximize.rs"
    shutil.copy(before, core / "src/algorithms/maximize.rs")
    manifest = (ROOT / "crates/core/Cargo.toml").read_text().split("[[bench]]")[0]
    (core / "Cargo.toml").write_text(manifest)
    with (core / "src/lib.rs").open("a") as out:
        out.write('\nmod review_flags;\n')
    (core / "src/review_flags.rs").write_text('''
+#[inline]
+fn bit(name: &str) -> u16 {
+    match name {
+        "early" => 1, "reuse" => 2, "components" => 4, "matching" => 8,
+        "shared" => 16, "retain" => 32, "dense" => 64, "bottom" => 128,
+        "wide" => 256, "subset" => 512, "prepare" => 1024, _ => 0,
+    }
+}
+#[inline]
+pub(crate) fn enabled(name: &str) -> bool {
+    static FLAGS: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
+    let flags = FLAGS.get_or_init(|| {
+        std::env::var("RE_PROFILE_VARIANT").unwrap_or_default()
+            .split('+').map(bit).fold(0, |a, b| a | b)
+    });
+    flags & bit(name) != 0
+}
+'''.replace("\n+", "\n"))
    combination = core / "src/algorithms/maximize/combination.rs"
    source = patch_source(combination.read_text())
    begin = source.index("    let mut choices = Vec::new();")
    end = source.index("    if !tracking && compatible.iter().all", begin)
    choice_code = source[begin:end]
    source = source[:begin] + source[end:]
    marker = "    let mut intersections = Vec::with_capacity"
    source = source.replace(marker, choice_code + '''    if crate::review_flags::enabled("early") && choices.is_empty() {
        return Ok(Vec::new());
    }
''' + marker, 1)
    # Both variants discover joins first; only 'early' skips preparation for an
    # empty choice set. Confirm selected changes against the genuinely unchanged
    # production executable as well, rather than treating this branch harness
    # as an exact copy of its instruction/allocation order.
    combination.write_text(source)
    for name in ["alignments", "components"]:
        shutil.copy(HERE / f"review_{name}.rs", core / f"src/algorithms/maximize/review_{name}.rs")
        with (core / "src/algorithms/maximize.rs").open("a") as out:
            out.write(f'\nmod review_{name};\n')
    replace(combination, "    let conflicts = dominated_exchanges(", '''    if crate::review_flags::enabled("prepare") && !compatible.iter().all(|&b| b) {
        let rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();
        let columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();
        let components = super::review_components::Components::new(&rows, &columns, &compatible, cancellation)?;
        choices.retain(|&(i, j, _)| components.allows(i, j));
        if choices.is_empty() { return Ok(Vec::new()); }
    }
    let conflicts = dominated_exchanges(''')
    replace(combination, "    for (i, j, union) in choices {\n        let mut rows:", '''    let mut alignment = if crate::review_flags::enabled("reuse") {
        Some(super::review_alignments::AlignmentSearch::new(&compatible, &conflicts, cancellation)?)
    } else { None };
    let mut row_scratch: Vec<_> = a.iter().map(|(_, n)| *n).collect();
    let mut column_scratch: Vec<_> = b.iter().map(|(_, n)| *n).collect();
    let components = if crate::review_flags::enabled("components") && !compatible.iter().all(|&b| b) {
        Some(super::review_components::Components::new(&row_scratch, &column_scratch, &compatible, cancellation)?)
    } else { None };
    for (i, j, union) in choices {
        if components.as_ref().is_some_and(|c| !c.allows(i, j)) { continue; }\n        let mut rows:''')
    replace(combination, "        let mut rows: Vec<_> = a.iter().map(|(_, n)| *n).collect();\n        let mut columns: Vec<_> = b.iter().map(|(_, n)| *n).collect();\n        rows[i] -= 1;\n        columns[j] -= 1;", '''        let mut rows = Vec::new();
        let mut columns = Vec::new();
        if alignment.is_none() {
            rows.extend(a.iter().map(|(_, n)| *n));
            columns.extend(b.iter().map(|(_, n)| *n));
            rows[i] -= 1; columns[j] -= 1;
        }''')
    replace(combination, "        visit_alignments_avoiding_with(\n            &rows,\n            &columns,\n            &compatible,\n            &conflicts,\n            cancellation,\n            |counts| {", "        let visitor = |counts: &[u32]| {")
    replace(combination, "            },\n        )?;", '''            };
        if let Some(alignment) = &mut alignment {
            row_scratch[i] -= 1; column_scratch[j] -= 1;
            alignment.visit_with(&row_scratch, &column_scratch, visitor)?;
            row_scratch[i] += 1; column_scratch[j] += 1;
        } else {
            visit_alignments_avoiding_with(&rows, &columns, &compatible, &conflicts, cancellation, visitor)?;
        }''')
    engine = core / "src/algorithms/maximize/engine.rs"
    replace(engine, "        seeds.sort();", '''        let shared_allowed = !crate::review_flags::enabled("bottom") || {
            let mut values = seeds.iter().flat_map(|row| row.multiplicities().iter().map(|(v, _)| v));
            if let Some(first) = values.next() {
                let mut bottom = first.clone();
                for value in values {
                    cancellation.check()?;
                    bottom = operations.intersection(&bottom, value);
                }
                operations.is_admissible(&bottom)
            } else { false }
        };
        seeds.sort();''')
    replace(engine, "                let results = map_with_progress_with_minimum(", '''                let shared = (!tracking && crate::review_flags::enabled("shared"))
                    .then_some(shared_allowed).filter(|&b| b)
                    .map(|_| std::sync::Mutex::new(BTreeSet::new()));
                let results = map_with_progress_with_minimum(''')
    replace(engine, "                            &seen,\n                        )", "                            &seen,\n                            shared.as_ref(),\n                        )")
    replace(combination, "    known: &BTreeSet<Configuration<T>>,", "    known: &BTreeSet<Configuration<T>>,\n    shared: Option<&std::sync::Mutex<BTreeSet<Configuration<T>>>>,")
    replace(combination, "                if !seen.insert(configuration.clone()) {", '''                if let Some(shared) = shared {
                    if !shared.lock().unwrap().insert(configuration.clone()) {
                        return Ok(());
                    }
                }
                if !seen.insert(configuration.clone()) {''')
    replace(combination, "    for (i, j, union) in choices {\n        if components.as_ref()", '''    let shared = shared.filter(|_| !crate::review_flags::enabled("dense")
        || compatible.iter().all(|&b| b));
    for (i, j, union) in choices {\n        if components.as_ref()''')
    # Remember even locally rejected rows if explicitly selected. The history
    # update must happen AFTER survivor deduplication, or new_candidates would
    # erroneously reject every newly generated survivor.
    replace(engine, "                changed |= current.merge(fresh, operations, &cancellation, &mut observe)?;", '''                if crate::review_flags::enabled("retain") {
                    if let Some(shared) = shared {
                        seen.extend(shared.into_inner().unwrap());
                    }
                }
                changed |= current.merge(fresh, operations, &cancellation, &mut observe)?;''')
    labels = core / "src/model/labels.rs"
    replace(labels, "        let mut right = 0;", '''        if crate::review_flags::enabled("subset") && self.len() > other.len() {
            return false;
        }
        let mut right = 0;''')
    matching = core / "src/algorithms/configurations/matching.rs"
    replace(matching, "    let mut compatible_edges = 0usize;", '''    let cache = crate::review_flags::enabled("matching")
        && offered.len().saturating_mul(allowed.len()) <= 128;
    let mut bits = 0u128;
    let mut compatible_edges = 0usize;''')

    replace(matching, "        for (target, n) in allowed {", "        for (j, (target, n)) in allowed.iter().enumerate() {")
    replace(matching, "                compatible_edges += 1;", '''                compatible_edges += 1;
                if cache { bits |= 1u128 << (i * allowed.len() + j); }''')
    text = matching.read_text()
    old = "    for (value, count) in offered {\n        cancellation.check()?;\n        if *count == 0"
    assert text.count(old) == 2
    matching.write_text(text.replace(old, "    for (i, (value, count)) in offered.iter().enumerate() {\n        cancellation.check()?;\n        if *count == 0"))
    replace(matching, "        for ((target, n), &destination) in allowed.iter().zip(&destinations) {", "        for (j, ((target, n), &destination)) in allowed.iter().zip(&destinations).enumerate() {")
    replace(matching, "            if *n > 0 && compatible(value, target) {\n                network.add_edge", '''            if *n > 0 && (if cache { bits & (1u128 << (i * allowed.len() + j)) != 0 }
                else { compatible(value, target) }) {
                network.add_edge''')
    packed = core / "src/algorithms/maximize/packed.rs"
    wider = packed.read_text().split("#[cfg(test)]")[0].replace("u64", "u128")
    wider = wider.replace("alphabet.len() > 64", "alphabet.len() > 128")
    (core / "src/algorithms/maximize/review_wide.rs").write_text(wider)
    with (core / "src/algorithms/maximize.rs").open("a") as out:
        out.write('\nmod review_wide;\n')
    replace(packed, "        if alphabet.len() > 64 {\n            return Ok(None);", '''        if alphabet.len() > 64 {
            if crate::review_flags::enabled("wide") {
                return super::review_wide::maximum_with(constraint, computation);
            }
            return Ok(None);''')
    (directory / "Cargo.toml").write_text(f'''[package]
name = "re-review"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
round-eliminator-3 = {{ path = "core" }}
round-eliminator-lib = {{ path = {json.dumps(str(ROOT.parent / "round-eliminator/round-eliminator-lib"))}, default-features = false, features = ["onlyrust"] }}
serde_json = "1"
rayon = "1"
mimalloc = {{ version = "=0.1.52", features = ["v2"] }}
libmimalloc-sys = {{ version = "=0.1.49", features = ["extended"] }}
[features]
mimalloc = []
mimalloc-v2 = ["mimalloc"]
[[bin]]
name = "re-review"
path = {json.dumps(str(HERE / "main.rs"))}
''')
    shutil.copy(HERE / "Cargo.lock", directory / "Cargo.lock")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--build", action="store_true")
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-target-linux"))
    options = parser.parse_args()
    prepare(options.directory.resolve())
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(options.directory.resolve() / "Cargo.toml"), "--features", "mimalloc-v2",
            "--target-dir", str(options.target_dir)], check=True)


if __name__ == "__main__":
    main()
