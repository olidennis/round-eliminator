#!/usr/bin/env python3
"""Compare temporary pruning prototypes sequentially and check full results."""

import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
BASE = "batch-256+capacity-first+recent+swap"
CASES = {
    "matching": "matching-10.json",
    "test-all-second": "fixture-2.json",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("/tmp/re3-target-linux/release/re-investigation"))
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--workers", type=int, default=10)
    parser.add_argument("--case", choices=CASES)
    parser.add_argument("--variant", action="append")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--stage", default="comparison")
    options = parser.parse_args()
    if options.samples < 1 or options.workers < 1:
        parser.error("samples and workers must be positive")
    large = "batch-4096+capacity-first+recent+swap"
    variants = options.variant or [BASE, large, large + "+parallel-antichain"]
    cases = [options.case] if options.case else CASES
    timings = {}
    with tempfile.TemporaryDirectory(prefix="re3-pruning-result-") as directory:
        result_path = Path(directory) / "maximum.json"
        for case in cases:
            expected = None
            for sample in range(1, options.samples + 1):
                # Alternate order to avoid always giving one variant the first
                # run. Processes and cases never run concurrently.
                order = variants if sample % 2 else list(reversed(variants))
                for variant in order:
                    environment = dict(os.environ, RE_NUM_THREADS=str(options.workers),
                        RAYON_NUM_THREADS=str(options.workers), RE_PROFILE_VARIANT=variant,
                        RE_PROFILE_RESULT=str(result_path))
                    result = subprocess.run([str(options.binary.resolve()), case, "profile-new", "1",
                        str(HERE / "investigation" / CASES[case])], env=environment,
                        check=True, capture_output=True, text=True)
                    maximum = json.loads(result_path.read_text())
                    if expected is None:
                        expected = maximum
                    elif maximum != expected:
                        raise RuntimeError(f"Different maximum: {case}, {variant}, sample {sample}")
                    row = json.loads(result.stdout)
                    row.update(variant=variant, sample=sample, workers=options.workers, stage=options.stage)
                    with options.output.open("a") as output:
                        output.write(json.dumps(row, sort_keys=True) + "\n")
                    timings.setdefault((case, variant), []).append(row["milliseconds"])
                    print(case, sample, variant, f'{row["milliseconds"]:.3f} ms', flush=True)
    for (case, variant), values in timings.items():
        print(case, variant, f"median {statistics.median(values):.3f} ms", flush=True)


if __name__ == "__main__":
    main()
