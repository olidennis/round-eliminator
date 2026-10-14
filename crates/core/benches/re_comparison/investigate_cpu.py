#!/usr/bin/env python3
"""Instrument the frozen snapshot-pruning baseline to separate kernel costs.

Uses configured mimalloc v2 and temporary copies; leaves production unchanged.
RE_PROFILE_BATCH overrides generation batch size, and the serial-pruning variant
disables parallel antichain filtering while generation remains parallel. Restore
the pre-live sources so historical variants remain reproducible after integration.
"""

import argparse
from pathlib import Path
import shutil
import subprocess

from investigate_current import prepare
from investigate import ROOT, replace


def prepare_production(directory, counts, fixed_batches=False):
    # Reuse the benchmark driver, allocator setup, phase counters and old hooks.
    prepare(directory, counts)
    core = directory / "core/src/algorithms"
    source = ROOT / "crates/core/src/algorithms"
    for name in ["maximize/engine.rs", "maximize/combination.rs",
                 "maximize/antichain.rs", "parallel.rs", "configurations.rs",
                 "configurations/alignments.rs", "configurations/matching.rs"]:
        shutil.copy(source / name, core / name)
    # The review changed combination signatures, alignment exports and matching.
    # Keep the exact earlier kernels rather than mixing them with the archived
    # snapshot-pruning engine or silently changing historical measurements.
    review = ROOT / "crates/core/benches/re_comparison/investigation/baseline/2026-10-13/review"
    for name in ["algorithms/maximize/combination.rs", "algorithms/configurations.rs",
                 "algorithms/configurations/alignments.rs", "algorithms/configurations/matching.rs",
                 "model/labels.rs"]:
        shutil.copy(review / name, directory / "core/src" / name)
    # Reproduce the snapshot-pruning baseline after live pruning is integrated.
    # New prototypes are inserted by investigate_packed.py after this restoration.
    baseline = ROOT / "crates/core/benches/re_comparison/investigation/baseline/2026-10-13/pre-live"
    for filename, target in {"maximize.rs": "maximize.rs", "engine.rs": "maximize/engine.rs",
                             "antichain.rs": "maximize/antichain.rs", "parallel.rs": "parallel.rs"}.items():
        shutil.copy(baseline / filename, core / target)
    engine = core / "maximize/engine.rs"
    if fixed_batches:
        # Keep the earlier batch-size experiments reproducible after production
        # gains its adaptive policy; all other integrated kernels are identical.
        shutil.copy(ROOT / "crates/core/benches/re_comparison/investigation/baseline/2026-10-13/fixed-batch-engine.rs", engine)
        replace(engine, "    let tracking = observe.is_some();", '''    let batch_size = std::env::var("RE_PROFILE_BATCH").ok()
        .map(|value| value.parse().unwrap()).unwrap_or(batch_size);
    let tracking = observe.is_some();''')
    else:
        replace(engine, "    let tracking = observe.is_some();", '''    let fixed_batch = std::env::var_os("RE_PROFILE_BATCH").is_some();
    let batch_capacity = std::env::var("RE_PROFILE_BATCH").ok()
        .map(|value| value.parse().unwrap()).unwrap_or(batch_capacity);
    let tracking = observe.is_some();''')
        replace(engine, "(!tracking && can_parallelize()).then", "(!fixed_batch && !tracking && can_parallelize()).then")
    replace(engine, "            let snapshot = current.as_slice().to_vec();", "            crate::profile_hooks::inc(14);\n            let snapshot = current.as_slice().to_vec();")
    replace(engine, "                let batch_started = completed;", "                let generation = crate::profile_hooks::stage(20, 43);\n                let batch_started = completed;")
    replace(engine, "                let fresh = new_candidates(results,", "                drop(generation);\n                let merge = crate::profile_hooks::stage(21, 45);\n                let fresh = new_candidates(results,")
    replace(engine, "                pairs.clear();", "                drop(merge);\n                pairs.clear();")
    replace(engine, "    let mut fresh = Vec::new();", "    let _history = crate::profile_hooks::timer(30);\n    let mut fresh = Vec::new();")
    replace(engine, "            fresh.push(candidate);", "            fresh.push(candidate);\n        } else { crate::profile_hooks::inc(6);")
    combination = core / "maximize/combination.rs"
    replace(combination, "    let a = left.multiplicities();", '''    #[cfg(feature="work-counts")]
    let _pair = crate::profile_hooks::combining();
    crate::profile_hooks::inc(0);
    let a = left.multiplicities();''')
    replace(combination, "        for (j, (y, _)) in b.iter().enumerate() {", "        for (j, (y, _)) in b.iter().enumerate() {\n            crate::profile_hooks::inc(10);")
    replace(combination, "    for (i, j, union) in choices {\n        let mut rows:", "    for (i, j, union) in choices {\n        crate::profile_hooks::inc(1);\n        let mut rows:")
    replace(combination, "            |counts| {", "            |counts| {\n                crate::profile_hooks::inc(2);\n                crate::profile_hooks::inc(3);")
    replace(combination, "                if known.contains(&configuration) {", "                if known.contains(&configuration) {\n                    crate::profile_hooks::inc(5);")
    replace(combination, "                if !seen.insert(configuration.clone()) {", "                if !seen.insert(configuration.clone()) {\n                    crate::profile_hooks::inc(4);")
    replace(combination, "    Ok(result)\n}", "    crate::profile_hooks::add(12, result.len() as u64);\n    Ok(result)\n}")
    antichain = core / "maximize/antichain.rs"
    replace(antichain, "        let parallel = can_parallelize()", "        let parallel = !crate::profile_hooks::enabled(\"serial-pruning\") && can_parallelize()")
    for old, new in [
        ("        for old in &self.recent {", "        for old in &self.recent {\n            crate::profile_hooks::inc(33);"),
        ("        for old in &self.configurations {", "        for old in &self.configurations {\n            crate::profile_hooks::inc(35);"),
        ("        for old in self.configurations.drain(..) {", "        for old in self.configurations.drain(..) {\n            crate::profile_hooks::inc(37);"),
        ("                    for old in cache.iter() {", "                    for old in cache.iter() {\n                        crate::profile_hooks::inc(33);"),
        ("                        for old in current {", "                        for old in current {\n                            crate::profile_hooks::inc(35);"),
        ("                        for candidate in &candidates {", "                        for candidate in &candidates {\n                            crate::profile_hooks::inc(37);"),
        ("                            if other != own", "                            if other != own { crate::profile_hooks::inc(38); }\n                            if other != own"),
    ]:
        replace(antichain, old, new)
    matching = core / "configurations/matching.rs"
    replace(matching, "    cancellation.check()?;\n    let degree", "    crate::profile_hooks::matched();\n    cancellation.check()?;\n    let degree")
    replace(matching, "    let matched = dinics", "    crate::profile_hooks::inc(9);\n    let matched = dinics")
    parallel = core / "parallel.rs"
    replace(parallel, "                    .for_each(|batch| {", '''                    .for_each(|batch| {
                        #[cfg(feature="work-counts")]
                        let _flush = crate::profile_hooks::Flush;''')
    manifest = directory / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace("re-current-", "re-cpu-"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--counts", action="store_true")
    parser.add_argument("--fixed-batches", action="store_true",
        help="Restore the pre-adaptive engine for historical batch experiments")
    parser.add_argument("--build", action="store_true")
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-target-linux"))
    options = parser.parse_args()
    prepare_production(options.directory.resolve(), options.counts, options.fixed_batches)
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(options.directory.resolve()/"Cargo.toml"), "--target-dir", str(options.target_dir)], check=True)


if __name__ == "__main__":
    main()
