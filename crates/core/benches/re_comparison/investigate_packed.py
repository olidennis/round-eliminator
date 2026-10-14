#!/usr/bin/env python3
"""Test packed set operations and pruning concurrency in temporary copies."""

import argparse
from pathlib import Path
import shutil
import subprocess

from investigate import HERE, replace
from investigate_cpu import prepare_production


def prepare_packed(directory):
    prepare_production(directory, False)
    algorithms = directory / "core/src/algorithms"
    shutil.copy(HERE / "packed_sets.rs", algorithms / "maximize/packed_sets.rs")
    shutil.copy(HERE / "live_pruning.rs", algorithms / "maximize/live_pruning.rs")
    maximize = algorithms / "maximize.rs"
    replace(maximize, "mod engine;", "mod engine;\nmod packed_sets;\nmod live_pruning;")
    replace(maximize, "    let maximum = maximize_custom_with(", '''    let packed = if crate::profile_hooks::enabled("packed") {
        packed_sets::maximum_with(constraint, computation)?
    } else { None };
    let maximum = if let Some(maximum) = packed { maximum } else { maximize_custom_with(''')
    replace(maximize, "    )?;\n    let mut result = Vec::with_capacity(maximum.len());", "    )? };\n    let mut result = Vec::with_capacity(maximum.len());")
    antichain = algorithms / "maximize/antichain.rs"
    replace(antichain, "    recent: Cache<T>,", "    recent: Cache<T>,\n    live_cost_ns: u64,")
    replace(antichain, "            recent: VecDeque::new(),", "            recent: VecDeque::new(),\n            live_cost_ns: 0,")
    replace(antichain, "        let candidates = if parallel {", '''        if parallel && observe.is_none() && crate::profile_hooks::enabled("live") {
            let capacity = std::env::var("RE_PROFILE_PRUNE_WORKERS").ok()
                .map(|value| value.parse::<usize>().unwrap()).unwrap_or(worker_count());
            let workers = if let Ok(target) = std::env::var("RE_PROFILE_PRUNE_TARGET_MS") {
                let target_ns = target.parse::<u128>().unwrap() * 1_000_000;
                ((candidates.len() as u128 * self.live_cost_ns as u128).div_ceil(target_ns) as usize)
                    .clamp(2.min(capacity), capacity)
            } else { capacity };
            let (changed, cost) = super::live_pruning::merge_with(
                &mut self.configurations, candidates, &mut self.caches, operations, cancellation,
                workers)?;
            self.live_cost_ns = cost;
            return Ok(changed);
        }
        let candidates = if parallel {''')
    replace(antichain, "        let workers = worker_count();", '''        let workers = std::env::var("RE_PROFILE_PRUNE_WORKERS").ok()
            .map(|value| value.parse::<usize>().unwrap()).unwrap_or(worker_count());''')
    replace(antichain, "        let chunk_size = count.div_ceil(worker_count());", '''        let workers = std::env::var("RE_PROFILE_PRUNE_WORKERS").ok()
            .map(|value| value.parse::<usize>().unwrap()).unwrap_or(worker_count());
        let chunk_size = count.div_ceil(workers);''')
    parallel = algorithms / "parallel.rs"
    replace(parallel, "            scope.spawn(move |_| {", '''            let minimum = if count >= 512 {
                std::env::var("RE_PROFILE_GRAIN").ok().map(|value| value.parse::<usize>().unwrap()).unwrap_or(minimum)
            } else { minimum };
            scope.spawn(move |_| {''')
    replace(parallel, ".with_max_len(128)", ".with_max_len(128.max(minimum))")
    manifest = directory / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace("re-cpu-", "re-packed-"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--build", action="store_true")
    options = parser.parse_args()
    prepare_packed(options.directory.resolve())
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(options.directory.resolve()/"Cargo.toml"), "--target-dir", "/tmp/re3-target-linux"], check=True)


if __name__ == "__main__":
    main()
