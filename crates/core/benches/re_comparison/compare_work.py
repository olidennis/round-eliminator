#!/usr/bin/env python3
"""Compare uninstrumented maximization wall time and aggregate process CPU time."""

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
CASES = {"matching": ("matching-10.json", 30), "test-all-second": ("fixture-2.json", 1)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--workers", nargs="+", type=int, default=[1, 4, 10])
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--case", choices=CASES)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    if options.samples < 1 or not options.workers or min(options.workers) < 1:
        parser.error("samples and workers must be positive")
    options.output.parent.mkdir(parents=True, exist_ok=True)
    summaries = {}
    # Only one measured child exists at a time. Differences in RUSAGE_CHILDREN
    # include CPU time of every thread in that child, including the coordinator.
    with tempfile.TemporaryDirectory(prefix="re3-work-") as directory, options.output.open("w") as output:
        output.write(json.dumps({"event": "environment", "platform": platform.platform(),
            "cpus": os.cpu_count(), "workers": options.workers, "samples": options.samples,
            "timing": "kernel wall time; process user+system CPU time divided by repetitions",
            "energy": "not measured"}) + "\n")
        result_path = Path(directory) / "maximum.json"
        cases = [options.case] if options.case else CASES
        for case in cases:
            expected = None
            snapshot, repetitions = CASES[case]
            for sample in range(1, options.samples + 1):
                order = options.workers if sample % 2 else list(reversed(options.workers))
                for workers in order:
                    environment = dict(os.environ, RAYON_NUM_THREADS=str(workers))
                    before = resource.getrusage(resource.RUSAGE_CHILDREN)
                    start = time.perf_counter()
                    result = subprocess.run([str(options.binary.resolve()), case, "resource-new",
                        str(repetitions), str(HERE / "investigation" / snapshot), str(result_path)],
                        env=environment, capture_output=True, text=True, check=True)
                    process_wall = time.perf_counter() - start
                    after = resource.getrusage(resource.RUSAGE_CHILDREN)
                    user = after.ru_utime - before.ru_utime
                    system = after.ru_stime - before.ru_stime
                    kernels = [json.loads(line) for line in result.stdout.splitlines()]
                    assert len(kernels) == repetitions
                    maximum = json.loads(result_path.read_text())
                    if expected is None:
                        expected = maximum
                    elif maximum != expected:
                        raise RuntimeError(f"Different maximum: {case}, {workers} workers, sample {sample}")
                    row = {"event": "measured", "case": case, "workers": workers,
                        "sample": sample, "repetitions": repetitions,
                        "kernel_wall_ms": statistics.mean(k["milliseconds"] for k in kernels),
                        "cpu_ms": (user + system) * 1000 / repetitions,
                        "user_ms": user * 1000 / repetitions,
                        "system_ms": system * 1000 / repetitions,
                        "process_wall_ms": process_wall * 1000 / repetitions,
                        "maxima": kernels[-1]["maxima"], "equivalent": True}
                    output.write(json.dumps(row, sort_keys=True) + "\n")
                    output.flush()
                    summaries.setdefault((case, workers), []).append(row)
                    print(f"{case}, sample {sample}, workers {workers}: "
                        f"wall {row['kernel_wall_ms']:.3f} ms, CPU {row['cpu_ms']:.3f} ms", flush=True)
    for (case, workers), rows in summaries.items():
        print(case, workers, {key: statistics.median(r[key] for r in rows)
            for key in ["kernel_wall_ms", "cpu_ms", "user_ms", "system_ms"]}, flush=True)


if __name__ == "__main__":
    main()
