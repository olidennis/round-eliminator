#!/usr/bin/env python3
"""Benchmark adaptive generation batches using temporary production copies.

RE_PROFILE_ADAPTIVE_MS sets a target generation duration per batch (512--4096
pairs). RE_PROFILE_BUFFERED defers global pruning to a 4096-pair boundary while
publishing generated survivors to duplicate history after every generation batch.
RE_PROFILE_STICKY lets batch sizes shrink within a degree but not grow again,
preventing cheap stretches of an expensive closure from undoing the reduction.
RE_PROFILE_PARTIAL also learns from expensive partial batches at round ends.
Neither option changes production, the set algebra, or the closure's round seeds.
"""

import argparse
from pathlib import Path
import subprocess

from investigate_cpu import prepare_production
from investigate import replace


def prepare_adaptive(directory):
    prepare_production(directory, False, fixed_batches=True)
    engine = directory / "core/src/algorithms/maximize/engine.rs"
    replace(engine, "    let tracking = observe.is_some();", '''    let adaptive_ns = std::env::var("RE_PROFILE_ADAPTIVE_MS").ok()
        .map(|value| value.parse::<u128>().unwrap() * 1_000_000);
    let buffered = std::env::var_os("RE_PROFILE_BUFFERED").is_some();
    let sticky = std::env::var_os("RE_PROFILE_STICKY").is_some();
    let partial = std::env::var_os("RE_PROFILE_PARTIAL").is_some();
    let trace = std::env::var_os("RE_PROFILE_TRACE").is_some();
    let samples = std::cell::RefCell::new(Vec::new());
    let limit = std::cell::Cell::new(batch_size);
    let tracking = observe.is_some();''')
    replace(engine, "            let mut changed = false;", '''            let mut changed = false;
            let mut pending = Vec::new();
            let mut pending_pairs = 0usize;''')
    replace(engine, "    for seeds in degrees.values_mut() {", '''    for seeds in degrees.values_mut() {
        limit.set(batch_size);''')
    replace(engine, "                let generation = crate::profile_hooks::stage(20, 43);", '''                let started = std::time::Instant::now();
                let generation = crate::profile_hooks::stage(20, 43);''')
    replace(engine, "                drop(generation);", '''                drop(generation);
                if let Some(target) = adaptive_ns {
                    // Ignore short round tails; bound each adjustment to a factor
                    // of two so a transient cannot collapse the next batch.
                    if pairs.len() == limit.get() || (partial && started.elapsed().as_nanos() >= target) {
                        let desired = (pairs.len() as u128 * target / started.elapsed().as_nanos().max(1))
                            .min(usize::MAX as u128) as usize;
                        let next = desired.clamp(limit.get() / 2, limit.get() * 2).clamp(512, 4096);
                        limit.set(if sticky { next.min(limit.get()) } else { next });
                    }
                }''')
    replace(engine, "                let fresh = new_candidates(results, &mut seen, &cancellation)?;", '''                let fresh = new_candidates(results, &mut seen, &cancellation)?;
                if trace {
                    samples.borrow_mut().push((snapshot[0].degree(), pairs.len(), snapshot.len(), work,
                        started.elapsed().as_nanos(), fresh.len(), seen.len(), limit.get()));
                }''')
    replace(engine,
        "                changed |= current.merge(fresh, operations, &cancellation, &mut observe)?;",
        '''                if buffered {
                    pending.extend(fresh);
                    pending_pairs += pairs.len();
                    if pending_pairs >= batch_size {
                        changed |= current.merge(std::mem::take(&mut pending), operations, &cancellation, &mut observe)?;
                        pending_pairs = 0;
                    }
                } else {
                    changed |= current.merge(fresh, operations, &cancellation, &mut observe)?;
                }''')
    replace(engine, "                    if pairs.len() == batch_size {", "                    if pairs.len() >= limit.get() {")
    replace(engine, "            if !changed {", '''            if !pending.is_empty() {
                let merge = crate::profile_hooks::stage(21, 45);
                changed |= current.merge(pending, operations, &cancellation, &mut observe)?;
                drop(merge);
            }
            if !changed {''')
    replace(engine, "    Ok(all)", '''    if trace {
        eprintln!("BATCH_PROFILE:{}", serde_json::to_string(&*samples.borrow()).unwrap());
    }
    Ok(all)''')
    manifest = directory / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace("re-cpu-", "re-adaptive-"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--build", action="store_true")
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-target-linux"))
    options = parser.parse_args()
    prepare_adaptive(options.directory.resolve())
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(options.directory.resolve()/"Cargo.toml"), "--target-dir", str(options.target_dir)], check=True)


if __name__ == "__main__":
    main()
