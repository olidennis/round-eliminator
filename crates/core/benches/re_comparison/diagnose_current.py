#!/usr/bin/env python3
"""Run sequential, controlled current-kernel diagnostics and compare full results."""

import argparse
import json
import os
from pathlib import Path
import resource
import statistics
import subprocess
import time

HERE = Path(__file__).resolve().parent
CASES = {
    "basic": (HERE / "basic-fixed-point.txt", "fixed-point"),
    "matching": (HERE / "investigation/matching-10.json", "resource"),
    "large": (HERE / "investigation/fixture-2.json", "resource"),
}
VARIANTS = {
    "live": {"RE_PROFILE_VARIANT": "live"},
    "live-prune2": {"RE_PROFILE_VARIANT": "live", "RE_PROFILE_PRUNE_WORKERS": "2"},
    "live-adaptive20": {"RE_PROFILE_VARIANT": "live", "RE_PROFILE_PRUNE_TARGET_MS": "20"},
    "live-adaptive2": {"RE_PROFILE_VARIANT": "live", "RE_PROFILE_PRUNE_TARGET_MS": "2"},
    "live-adaptive5": {"RE_PROFILE_VARIANT": "live", "RE_PROFILE_PRUNE_TARGET_MS": "5"},
    "live-adaptive8": {"RE_PROFILE_VARIANT": "live", "RE_PROFILE_PRUNE_TARGET_MS": "8"},
    "live-adaptive40": {"RE_PROFILE_VARIANT": "live", "RE_PROFILE_PRUNE_TARGET_MS": "40"},
    "packed-live-adaptive20-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_PRUNE_TARGET_MS": "20", "RE_PROFILE_GRAIN": "256"},
    "packed-live-adaptive2-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_PRUNE_TARGET_MS": "2", "RE_PROFILE_GRAIN": "256"},
    "packed-live-adaptive5-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_PRUNE_TARGET_MS": "5", "RE_PROFILE_GRAIN": "256"},
    "packed-live-adaptive8-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_PRUNE_TARGET_MS": "8", "RE_PROFILE_GRAIN": "256"},
    "packed-live-adaptive40-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_PRUNE_TARGET_MS": "40", "RE_PROFILE_GRAIN": "256"},
    "packed-live": {"RE_PROFILE_VARIANT": "packed+live"},
    "packed-live-grain128": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_GRAIN": "128"},
    "packed-live-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_GRAIN": "256"},
    "packed-live-prune2-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_GRAIN": "256", "RE_PROFILE_PRUNE_WORKERS": "2"},
    "packed-live-prune4-grain256": {"RE_PROFILE_VARIANT": "packed+live", "RE_PROFILE_GRAIN": "256", "RE_PROFILE_PRUNE_WORKERS": "4"},
    "packed": {"RE_PROFILE_VARIANT": "packed"},
    "packed-prune2": {"RE_PROFILE_VARIANT": "packed", "RE_PROFILE_PRUNE_WORKERS": "2"},
    "packed-prune4": {"RE_PROFILE_VARIANT": "packed", "RE_PROFILE_PRUNE_WORKERS": "4"},
    "packed-prune6": {"RE_PROFILE_VARIANT": "packed", "RE_PROFILE_PRUNE_WORKERS": "6"},
    "packed-grain128": {"RE_PROFILE_VARIANT": "packed", "RE_PROFILE_GRAIN": "128"},
    "packed-prune4-grain64": {"RE_PROFILE_VARIANT": "packed", "RE_PROFILE_PRUNE_WORKERS": "4", "RE_PROFILE_GRAIN": "64"},
    "packed-prune4-grain128": {"RE_PROFILE_VARIANT": "packed", "RE_PROFILE_PRUNE_WORKERS": "4", "RE_PROFILE_GRAIN": "128"},
    "packed-prune6-grain128": {"RE_PROFILE_VARIANT": "packed", "RE_PROFILE_PRUNE_WORKERS": "6", "RE_PROFILE_GRAIN": "128"},
    "packed-serial-pruning": {"RE_PROFILE_VARIANT": "packed+serial-pruning"},
    "packed-serial-pruning-grain128": {"RE_PROFILE_VARIANT": "packed+serial-pruning", "RE_PROFILE_GRAIN": "128"},
    "packed-serial-pruning-grain256": {"RE_PROFILE_VARIANT": "packed+serial-pruning", "RE_PROFILE_GRAIN": "256"},
    "baseline": {},
    "old": {},
    "chunk32": {"RE_PROFILE_CHUNK": "32"},
    "chunk128": {"RE_PROFILE_CHUNK": "128"},
    "serial-pruning": {"RE_PROFILE_VARIANT": "serial-pruning"},
    "chunk32-serial-pruning": {"RE_PROFILE_VARIANT": "serial-pruning", "RE_PROFILE_CHUNK": "32"},
    "chunk32-batch512": {"RE_PROFILE_CHUNK": "32", "RE_PROFILE_BATCH": "512"},
    "union-prune": {"RE_PROFILE_VARIANT": "union-prune"},
    "early-known": {"RE_PROFILE_VARIANT": "early-known"},
    "union-known": {"RE_PROFILE_VARIANT": "union-prune+early-known"},
    "union-known-chunk32": {"RE_PROFILE_VARIANT": "union-prune+early-known", "RE_PROFILE_CHUNK": "32"},
    "early-known-chunk32": {"RE_PROFILE_VARIANT": "early-known", "RE_PROFILE_CHUNK": "32"},
    "combined-search": {"RE_PROFILE_VARIANT": "union-prune+early-known+conflict-search", "RE_PROFILE_CHUNK": "32"},
    "chunk128-peers-first": {"RE_PROFILE_VARIANT": "peers-first", "RE_PROFILE_CHUNK": "128"},
    "early-search-chunk32": {"RE_PROFILE_VARIANT": "early-known+conflict-search", "RE_PROFILE_CHUNK": "32"},
    "batch256": {"RE_PROFILE_BATCH": "256"},
    "batch512": {"RE_PROFILE_BATCH": "512"},
    "batch1024": {"RE_PROFILE_BATCH": "1024"},
    "batch2048": {"RE_PROFILE_BATCH": "2048"},
    "batch4096": {"RE_PROFILE_BATCH": "4096"},
    "adaptive5": {"RE_PROFILE_ADAPTIVE_MS": "5"},
    "adaptive10": {"RE_PROFILE_ADAPTIVE_MS": "10"},
    "adaptive20": {"RE_PROFILE_ADAPTIVE_MS": "20"},
    "adaptive10-buffered": {"RE_PROFILE_ADAPTIVE_MS": "10", "RE_PROFILE_BUFFERED": "1"},
    "adaptive10-sticky": {"RE_PROFILE_ADAPTIVE_MS": "10", "RE_PROFILE_STICKY": "1"},
    "adaptive20-sticky": {"RE_PROFILE_ADAPTIVE_MS": "20", "RE_PROFILE_STICKY": "1"},
    "adaptive10-sticky-buffered": {"RE_PROFILE_ADAPTIVE_MS": "10", "RE_PROFILE_STICKY": "1", "RE_PROFILE_BUFFERED": "1"},
    "adaptive5-partial": {"RE_PROFILE_ADAPTIVE_MS": "5", "RE_PROFILE_PARTIAL": "1"},
    "adaptive10-partial": {"RE_PROFILE_ADAPTIVE_MS": "10", "RE_PROFILE_PARTIAL": "1"},
    "adaptive10-sticky-partial": {"RE_PROFILE_ADAPTIVE_MS": "10", "RE_PROFILE_PARTIAL": "1", "RE_PROFILE_STICKY": "1"},
    "adaptive10-sticky-partial-buffered": {"RE_PROFILE_ADAPTIVE_MS": "10", "RE_PROFILE_PARTIAL": "1", "RE_PROFILE_STICKY": "1", "RE_PROFILE_BUFFERED": "1"},
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("/tmp/re3-target-linux/release/re-current-phases"))
    parser.add_argument("--case", choices=CASES, required=True)
    parser.add_argument("--variants", nargs="+", choices=VARIANTS, default=["baseline"])
    parser.add_argument("--workers", type=int, default=10)
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--iterations", type=int, default=1,
        help="Kernels per process for ordinary maximization; amortizes startup on small fixtures")
    parser.add_argument("--sample-start", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=1800)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    if options.iterations < 1 or (options.case == "basic" and options.iterations != 1):
        parser.error("iterations must be positive, and Basic requires one kernel per process")
    options.output.parent.mkdir(parents=True, exist_ok=True)
    artifacts = options.output.with_suffix(".results")
    artifacts.mkdir(parents=True, exist_ok=True)
    fixture, prefix = CASES[options.case]
    baseline_path = artifacts / "canonical.json"
    expected = json.loads(baseline_path.read_text()) if baseline_path.exists() else None
    with options.output.open("a") as output:
        for sample in range(options.sample_start, options.sample_start + options.samples):
            order = options.variants if sample % 2 else list(reversed(options.variants))
            for variant in order:
                environment = {k: v for k, v in os.environ.items() if not k.startswith("RE_PROFILE_")}
                environment.update(RE_NUM_THREADS=str(options.workers), RAYON_NUM_THREADS=str(options.workers))
                environment.update(VARIANTS[variant])
                result_path = artifacts / f"{variant}-{options.workers}-{sample}.json"
                print(f"Starting {options.case}, {variant}, {options.workers} workers, sample {sample}", flush=True)
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                start = time.perf_counter()
                process = subprocess.run([str(options.binary), options.case,
                    prefix + ("-old" if variant == "old" else "-new"), str(options.iterations), str(fixture), str(result_path)],
                    env=environment, capture_output=True, text=True, check=True, timeout=options.timeout)
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                result = json.loads(result_path.read_text())
                if expected is None:
                    expected = result
                    baseline_path.write_text(json.dumps(result))
                elif result != expected:
                    raise RuntimeError(f"Different complete result: {variant}/{options.workers}")
                updates = [json.loads(line) for line in process.stdout.splitlines()]
                row = dict(case=options.case, variant=variant, workers=options.workers,
                    sample=sample, iterations=options.iterations, binary=str(options.binary), equivalent=True,
                    wall_ms=statistics.mean(u["milliseconds"] for u in updates if "milliseconds" in u),
                    process_ms=(time.perf_counter()-start)*1000,
                    user_ms=(after.ru_utime-before.ru_utime)*1000/options.iterations,
                    system_ms=(after.ru_stime-before.ru_stime)*1000/options.iterations, updates=updates)
                row["cpu_ms"] = row["user_ms"] + row["system_ms"]
                output.write(json.dumps(row) + "\n")
                output.flush()
                print(f"Finished wall {row['wall_ms']/1000:.3f}s, CPU {row['cpu_ms']/1000:.3f}s (system {row['system_ms']/1000:.3f}s)", flush=True)


if __name__ == "__main__":
    main()
