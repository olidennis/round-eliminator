#!/usr/bin/env python3
"""Prepare four remaining maximization experiments without production flags."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
DRIVER = HERE.parent
ROOT = DRIVER.parents[3]
BASE = DRIVER / "investigation/baseline/2026-10-13/final-four"


def replace(path, old, new):
    text = path.read_text()
    assert text.count(old) == 1, (path, old, text.count(old))
    path.write_text(text.replace(old, new))


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=True)
    core = directory / "core"
    shutil.copytree(ROOT / "crates/core/src", core / "src", dirs_exist_ok=True)
    for path in BASE.rglob("*.rs"):
        shutil.copy(path, core / "src" / path.relative_to(BASE))
    (core / "Cargo.toml").write_text((ROOT / "crates/core/Cargo.toml").read_text().split("[[bench]]")[0])
    with (core / "src/lib.rs").open("a") as out:
        out.write('\nmod four_flags;\n')
    (core / "src/four_flags.rs").write_text('''
fn bit(name: &str) -> u16 { match name {
    "flow" => 1, "fixed" => 2, "dynamic" => 4, "pipe5" => 8, "pipe8" => 16,
    "compressed" => 32, "grain512" => 64, "grain1024" => 128, "buffered" => 256, "unitgrain" => 512, _ => 0,
} }
#[inline] pub(crate) fn enabled(name: &str) -> bool {
    static FLAGS: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
    let flags = FLAGS.get_or_init(|| std::env::var("RE_PROFILE_VARIANT").unwrap_or_default()
        .split('+').map(bit).fold(0, |a,b| a | b));
    flags & bit(name) != 0
}
''')
    maximum = core / "src/algorithms/maximize.rs"
    for name in ["sets", "history", "pipeline"]:
        shutil.copy(HERE / f"{name}.rs", core / f"src/algorithms/maximize/four_{name}.rs")
        with maximum.open("a") as out: out.write(f'\nmod four_{name};\n')
    shutil.copy(HERE / "flow.rs", core / "src/algorithms/configurations/four_flow.rs")
    with (core / "src/algorithms/configurations.rs").open("a") as out: out.write('\nmod four_flow;\n')
    matching = core / "src/algorithms/configurations/matching.rs"
    replace(matching, "    let mut network = Graph::<(), u32>::with_capacity(", '''    if crate::four_flags::enabled("flow") {
        return super::four_flow::solve_with(offered, allowed, compatible,
            cache.then_some(bits), compatible_edges, degree, cancellation);
    }
    let mut network = Graph::<(), u32>::with_capacity(''')
    packed = core / "src/algorithms/maximize/packed.rs"
    replace(packed, "        if alphabet.len() > 128 {\n            return Ok(None);", '''        if alphabet.len() > 128 {
            if crate::four_flags::enabled("fixed") || crate::four_flags::enabled("dynamic") {
                return super::four_sets::maximum_with(constraint, computation);
            }
            return Ok(None);''')
    engine = core / "src/algorithms/maximize/engine.rs"
    replace(engine, "    let cancellation = computation.cancellation.clone();", '''    let minimum = if minimum > 1 && crate::four_flags::enabled("grain1024") { 1024 }
        else if minimum > 1 && crate::four_flags::enabled("grain512") { 512 }
        else { minimum };
    let cancellation = computation.cancellation.clone();''')
    replace(engine, "        let mut seen: BTreeSet<_> = current.as_slice().iter().cloned().collect();",
        "        let mut seen: super::four_history::History<_> = current.as_slice().iter().cloned().collect();")
    replace(engine, "        seeds.sort();", '''        let minimum = if minimum > 1 && crate::four_flags::enabled("unitgrain")
            && seeds.iter().all(|row| row.multiplicities().iter().all(|(_, n)| *n == 1)) {
            512
        } else { minimum };
        seeds.sort();''')
    replace(engine, "    seen: &mut BTreeSet<Configuration<T>>,", "    seen: &mut super::four_history::History<T>,")
    combination = core / "src/algorithms/maximize/combination.rs"
    replace(combination, "    known: &BTreeSet<Configuration<T>>,", "    known: &super::four_history::History<T>,")
    # Trace and serial/wasm fall back to the normal complete generation batch.
    replace(engine, "                let results = map_with_progress_with_minimum(", '''                let overlap = !tracking && can_parallelize() && work >= 20_000
                    && (crate::four_flags::enabled("pipe5") || crate::four_flags::enabled("pipe8"));
                let results = if overlap {
                    let mut batch_seen: super::four_history::History<T> = std::iter::empty().collect();
                    let mut pending = Vec::new();
                    super::four_pipeline::visit(pairs.len(), minimum, &cancellation,
                        |index| {
                            let (a,b) = pairs[index];
                            combine(&snapshot[a], &snapshot[b], operations, &cancellation,
                                tracking, &seen, generated.as_ref())
                        },
                        |results, count| {
                            pending.extend(new_candidates(results, &mut batch_seen, &cancellation)?);
                            if !crate::four_flags::enabled("buffered") || pending.len() >= 1024 || count == pairs.len() {
                                changed |= current.merge(std::mem::take(&mut pending), operations, &cancellation, &mut observe)?;
                            }
                            completed = batch_started.saturating_add(count as u32);
                            pass_completed = pass_started.saturating_add(count as u64);
                            computation.report_work(Phase::Maximizing, completed,
                                WorkProgress { completed: pass_completed, total, pass })
                        })?;
                    // Every generated row is covered by the updated antichain.
                    // Preserve the duplicate history across generation batches.
                    seen.append(batch_seen);
                    Vec::new()
                } else { map_with_progress_with_minimum(''')
    replace(engine, "                )?;\n                #[cfg(not(target_arch = \"wasm32\"))]\n                if let Some(started)",
        "                )? };\n                #[cfg(not(target_arch = \"wasm32\"))]\n                if let Some(started)")
    antichain = core / "src/algorithms/maximize/antichain.rs"
    replace(antichain, "            let capacity = worker_count();", '''            let capacity = if crate::four_flags::enabled("pipe8") { 2.min(worker_count()) }
                else if crate::four_flags::enabled("pipe5") { (worker_count() / 2).max(1) }
                else { worker_count() };''')
    (directory / "Cargo.toml").write_text(f'''[package]
name = "re-final-four"
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
name = "re-final-four"
path = {json.dumps(str(DRIVER / "main.rs"))}
''')
    shutil.copy(DRIVER / "Cargo.lock", directory / "Cargo.lock")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--build", action="store_true")
    options = parser.parse_args()
    prepare(options.directory.resolve())
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(options.directory.resolve() / "Cargo.toml"), "--features", "mimalloc-v2",
            "--target-dir", "/tmp/re3-target-linux"], check=True)


if __name__ == "__main__": main()
