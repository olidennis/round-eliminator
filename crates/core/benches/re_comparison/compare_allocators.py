#!/usr/bin/env python3
"""Compare system, jemalloc and mimalloc on old/new saturation kernels."""

import argparse
import json
import os
from pathlib import Path
import platform
import resource
import statistics
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
CASES = {
    "basic": (HERE / "basic-fixed-point.txt", "fixed-point", 1),
    "matching": (HERE / "investigation/matching-10.json", "resource", 30),
    "test-all-second": (HERE / "investigation/fixture-2.json", "resource", 1),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-target-linux"))
    parser.add_argument("--allocator", action="append", choices=["system", "jemalloc", "mimalloc", "mimalloc-v2"])
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--case", choices=CASES)
    parser.add_argument("--old-serial", action="store_true", help="Measure only the old native build using its serial loop")
    parser.add_argument("--implementation", action="append", choices=["old", "new"], help="Restrict the measured implementations")
    parser.add_argument("--workers", type=int, nargs="+", help="Restrict the existing worker-count settings")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=600, help="Per-process benchmark watchdog")
    options = parser.parse_args()
    if options.samples < 1 or options.timeout <= 0:
        parser.error("samples and timeout must be positive")
    options.output.parent.mkdir(parents=True, exist_ok=True)
    allocators = options.allocator or ["system", "jemalloc", "mimalloc"]
    settings = [("old", 1)] if options.old_serial else [("old", 10), ("new", 10), ("new", 1)]
    settings = [(kind, workers) for kind, workers in settings
        if (not options.implementation or kind in options.implementation)
        and (not options.workers or workers in options.workers)]
    if not settings:
        parser.error("the selected implementation and worker counts have no matching setting")
    variants = [(allocator, kind, workers) for allocator in allocators for kind, workers in settings]
    summaries = {}
    with tempfile.TemporaryDirectory(prefix="re3-allocator-result-") as directory, options.output.open("w") as output:
        output.write(json.dumps({"event": "environment", "platform": platform.platform(),
            "cpus": os.cpu_count(), "samples": options.samples, "allocators": allocators,
            "timing": "kernel wall time; aggregate child-process user+system CPU",
            "old_worker_note": "native build of existing wasm serial loop" if options.old_serial else
                "limit 10 creates 10 generation and 10 pruning workers"}) + "\n")
        result_path = Path(directory) / "result.json"
        for case in [options.case] if options.case else CASES:
            expected = None
            fixture, prefix, repetitions = CASES[case]
            for sample in range(1, options.samples + 1):
                order = variants if sample % 2 else list(reversed(variants))
                for allocator, kind, workers in order:
                    binary = options.target_dir / "release" / ("re-comparison" if allocator == "system" else "re-comparison-" + allocator)
                    if options.old_serial:
                        binary = binary.with_name(binary.name + "-old-serial")
                    environment = dict(os.environ, RE_NUM_THREADS=str(workers), RAYON_NUM_THREADS=str(workers))
                    print(f"Starting {case}, {allocator}, {kind}, {workers} workers, sample {sample}", flush=True)
                    before = resource.getrusage(resource.RUSAGE_CHILDREN)
                    start = time.perf_counter()
                    try:
                        result = subprocess.run([str(binary.resolve()), case, prefix + "-" + kind,
                            str(repetitions), str(fixture), str(result_path)], env=environment,
                            capture_output=True, text=True, check=True, timeout=options.timeout)
                    except subprocess.TimeoutExpired:
                        row = {"event": "timeout", "case": case, "allocator": allocator,
                            "implementation": kind, "workers": workers, "sample": sample,
                            "repetitions": repetitions, "process_seconds": options.timeout}
                        output.write(json.dumps(row, sort_keys=True) + "\n")
                        output.flush()
                        print(f"Timed out after {options.timeout:g} seconds; no complete result", flush=True)
                        continue
                    process_wall = time.perf_counter() - start
                    after = resource.getrusage(resource.RUSAGE_CHILDREN)
                    updates = [json.loads(line) for line in result.stdout.splitlines()]
                    kernels = [row for row in updates if row.get("event") not in ["prepared", "starting"]]
                    assert len(kernels) == repetitions
                    maximum = json.loads(result_path.read_text())
                    if expected is None:
                        expected = maximum
                    elif maximum != expected:
                        raise RuntimeError(f"Different complete result: {case}/{allocator}/{kind}/{workers}")
                    row = {"event": "measured", "case": case, "allocator": allocator,
                        "implementation": kind, "workers": workers, "sample": sample,
                        "repetitions": repetitions, "equivalent": True,
                        "kernel_wall_ms": statistics.mean(k["milliseconds"] for k in kernels),
                        "user_ms": (after.ru_utime - before.ru_utime) * 1000 / repetitions,
                        "system_ms": (after.ru_stime - before.ru_stime) * 1000 / repetitions,
                        "process_wall_ms": process_wall * 1000 / repetitions}
                    row["cpu_ms"] = row["user_ms"] + row["system_ms"]
                    for key in ["active_ms", "passive_ms", "active_rows", "passive_rows", "maxima"]:
                        if key in kernels[0]:
                            row[key] = statistics.mean(k[key] for k in kernels)
                    for update in updates:
                        if update.get("event") == "prepared":
                            row["preparation"] = update
                    output.write(json.dumps(row, sort_keys=True) + "\n")
                    output.flush()
                    summaries.setdefault((case, allocator, kind, workers), []).append(row)
                    print(f"Finished: wall {row['kernel_wall_ms']:.3f} ms, CPU {row['cpu_ms']:.3f} ms", flush=True)
    for key, rows in summaries.items():
        print(key, {field: statistics.median(row[field] for row in rows)
            for field in ["kernel_wall_ms", "cpu_ms", "user_ms", "system_ms"]}, flush=True)


if __name__ == "__main__":
    main()
