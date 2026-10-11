#!/usr/bin/env python3
"""Build the current library and compare its general and maximized modes.

Requires Cargo and (unless --native-only) Node and wasm-pack. Sources are never
modified. Run from anywhere; keep other CPU-heavy work idle during measurements.
"""

import argparse
import csv
import io
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
BENCH = HERE.parent


def run(arguments, **kwargs):
    return subprocess.run(arguments, cwd=ROOT, check=True, text=True, **kwargs)


def build(directory, toolchain, wasm, package):
    built = run(["cargo", f"+{toolchain}", "build", "-p", "round-eliminator-3",
                 "--release", "--bench", "zero_round", "--message-format=json"],
                stdout=subprocess.PIPE)
    native = next(item["executable"] for item in map(json.loads, built.stdout.splitlines())
                  if item.get("reason") == "compiler-artifact"
                  and item.get("target", {}).get("name") == "zero_round"
                  and item.get("executable"))
    if wasm and package is None:
        package = directory / "wasm"
        run(["wasm-pack", "build", "crates/wasm", "--target", "web", "--out-dir",
             str(package), "--out-name", "round_eliminator_3_wasm", "--release"],
            env={**os.environ, "RUSTUP_TOOLCHAIN": toolchain})
    return native, package


def measure(native, package):
    results = []
    for suite, filename, modes in [
        ("general", "zero_round_cases.json", ("general",)),
        ("maximized", "zero_round_maximized_cases.json", ("general", "maximized")),
    ]:
        cases = BENCH / "support" / filename
        samples = {}
        for order in (modes, tuple(reversed(modes))):
            for mode in order:
                print(f"Measuring native {suite}: {mode}", flush=True)
                measured = run([native], stdout=subprocess.PIPE, env={**os.environ,
                    "RE3_BENCH_CASES": str(cases), "RE3_PASSIVE_MODE": mode})
                for row in csv.DictReader(io.StringIO(measured.stdout)):
                    samples.setdefault((row["case"], mode), []).append(
                        float(row["median_ms_per_request"]))
        for (case, mode), values in samples.items():
            results.append((suite, "native", case, mode, statistics.median(values)))
        if package:
            print(f"Measuring wasm {suite}", flush=True)
            arguments = ["node", str(BENCH / "zero_round_wasm.mjs"), "--cases", str(cases)]
            if len(modes) > 1:
                arguments.append("--compare-modes")
            measured = run([*arguments, str(package)], stdout=subprocess.PIPE)
            for row in csv.DictReader(io.StringIO(measured.stdout)):
                mode = row["implementation"].rsplit(":", 1)[1]
                results.append((suite, "wasm", row["case"], mode,
                                float(row["median_ms_per_request"])))
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="1.89.0")
    parser.add_argument("--native-only", action="store_true")
    parser.add_argument("--wasm-package", type=Path, help="Use an already built release package")
    parser.add_argument("--output", type=Path, default=Path("passive-input-results.csv"))
    options = parser.parse_args()
    package = options.wasm_package.resolve() if options.wasm_package else None
    if options.native_only:
        package = None
    with tempfile.TemporaryDirectory(prefix="re3-passive-bench-") as temporary:
        native, package = build(Path(temporary), options.toolchain, not options.native_only, package)
        rows = measure(native, package)
    with options.output.open("w", newline="") as output:
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow(("suite", "target", "case", "implementation", "median_ms_per_request"))
        writer.writerows((*row[:4], f"{row[4]:.6f}") for row in rows)
    print(f"Results: {options.output.resolve()}")


if __name__ == "__main__":
    main()
