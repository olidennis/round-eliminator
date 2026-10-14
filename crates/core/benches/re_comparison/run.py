#!/usr/bin/env python3
"""Compare old/new native RE and maximization without modifying either library."""

import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import threading
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
OLD = ROOT.parent / "round-eliminator" / "round-eliminator-lib"


def build(temporary, toolchain, target, allocator="system", old_serial=False):
    package = Path(temporary)
    old_source = OLD
    binary_name = "re-comparison" if allocator == "system" else "re-comparison-" + allocator
    if old_serial:
        # Use the old, already-existing serial loop on native hardware with the
        # requested allocator. A one-worker native pipeline is still concurrent.
        old_source = package / "old-serial"
        shutil.copytree(OLD / "src", old_source / "src")
        manifest = (OLD / "Cargo.toml").read_text().split("[patch.crates-io]")[0]
        (old_source / "Cargo.toml").write_text(manifest)
        kernel = old_source / "src/algorithms/maximize.rs"
        source = kernel.read_text()
        native = '#[cfg(not(target_arch = "wasm32"))]'
        serial = '#[cfg(target_arch = "wasm32")]'
        assert source.count(native) == source.count(serial) == 1
        kernel.write_text(source.replace(native, '#[cfg(any())]').replace(serial, '#[cfg(all())]'))
        binary_name += "-old-serial"
    # A separate temporary package keeps the old crate out of RE3's normal
    # dependencies, lockfile, and workspace. SAT backends are unused by these
    # plain RE/maximization fixtures; onlyrust avoids building old MiniSat.
    (package / "Cargo.toml").write_text(f'''[package]
name = "re-comparison"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
round-eliminator-3 = {{ path = {json.dumps(str(ROOT / "crates/core"))} }}
round-eliminator-lib = {{ path = {json.dumps(str(old_source))}, default-features = false, features = ["onlyrust"] }}
serde_json = "1"
rayon = "1"
tikv-jemallocator = {{ version = "=0.6.1", optional = true }}
mimalloc = {{ version = "=0.1.52", optional = true }}
libmimalloc-sys = {{ version = "=0.1.49", features = ["extended"], optional = true }}
[features]
jemalloc = ["dep:tikv-jemallocator"]
mimalloc = ["dep:mimalloc", "dep:libmimalloc-sys"]
mimalloc-v2 = ["mimalloc", "mimalloc/v2"]
[[bin]]
name = {json.dumps(binary_name)}
path = {json.dumps(str(HERE / "main.rs"))}
''')
    if (HERE / "Cargo.lock").exists():
        (package / "Cargo.lock").write_bytes((HERE / "Cargo.lock").read_bytes())
    command = ["cargo", f"+{toolchain}", "build", "--release", "--manifest-path",
               str(package / "Cargo.toml"), "--target-dir", str(target)]
    if allocator != "system":
        command.extend(["--features", allocator])
    subprocess.run(command, check=True)
    (HERE / "Cargo.lock").write_bytes((package / "Cargo.lock").read_bytes())
    return target / ("release/" + binary_name)


def measure(binary, case, mode, steps, fixture, threads, timeout, output, sample):
    environment = {**os.environ, "RE_NUM_THREADS": str(threads), "RAYON_NUM_THREADS": str(threads),
                   "RE_BENCH_SAMPLE": str(sample)}
    process = subprocess.Popen([str(binary), case, mode, str(steps), str(fixture)],
        env=environment, stdout=subprocess.PIPE, text=True, start_new_session=True)
    last_activity = [time.monotonic()]
    stopped = threading.Event()

    def watchdog():
        while not stopped.wait(0.5):
            if time.monotonic() - last_activity[0] > timeout:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                return

    watcher = threading.Thread(target=watchdog, daemon=True)
    watcher.start()
    pending = None
    try:
        for line in process.stdout:
            row = json.loads(line)
            last_activity[0] = time.monotonic()
            row.update(threads=threads, sample=sample)
            if row["event"] == "starting":
                pending = row["step"]
                print(f"{case}: {mode}, sample {sample}, step {pending}", flush=True)
            else:
                pending = None
                output.write(json.dumps(row) + "\n")
                output.flush()
                print(json.dumps(row), flush=True)
        code = process.wait()
    finally:
        stopped.set()
        watcher.join()
        process.stdout.close()
    if code == -signal.SIGTERM:
        row = dict(event="timeout", case=case, mode=mode, step=pending,
            seconds=timeout, threads=threads, sample=sample)
        output.write(json.dumps(row) + "\n")
        output.flush()
        print(json.dumps(row), flush=True)
    elif code:
        raise RuntimeError(f"Benchmark failed ({code}): {case}/{mode}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="1.89.0")
    parser.add_argument("--allocator", choices=["system", "jemalloc", "mimalloc", "mimalloc-v2"], default="system")
    parser.add_argument("--old-serial", action="store_true", help="Build the old library using its existing serial saturation loop")
    parser.add_argument("--build-only", action="store_true", help="Build the comparison executable without running measurements")
    parser.add_argument("--threads", type=int, default=os.cpu_count())
    parser.add_argument("--steps", type=int, default=10, help="Matching sequence length")
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--sample-start", type=int, default=1)
    parser.add_argument("--append", action="store_true")
    parser.add_argument("--timeout", type=float, default=300, help="Benchmark watchdog per unfinished step; not an algorithm cutoff")
    parser.add_argument("--case", choices=["matching", "test-all-second", "both"], default="both")
    parser.add_argument("--modes", nargs="+", default=["old-re", "new-re", "old-button", "maximize"],
        choices=["old-re", "new-re", "old-button", "maximize", "maximize-cleaned"])
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-re-comparison-target"))
    parser.add_argument("--binary", type=Path, help="Reuse an already built benchmark")
    parser.add_argument("--output", type=Path, default=HERE / "results.jsonl")
    options = parser.parse_args()
    options.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="re3-re-comparison-") as temporary:
        binary = options.binary.resolve() if options.binary else build(temporary, options.toolchain, options.target_dir.resolve(), options.allocator, options.old_serial)
        if options.build_only:
            print(binary, flush=True)
            return
        matching = Path(temporary) / "matching.txt"
        matching.write_text("M U^9\nP^10\n\nM UP^9\nU^10\n")
        cases = [("matching", options.steps, matching), ("test-all-second", 2, HERE / "test-all-second.txt")]
        with options.output.open("a" if options.append else "w") as output:
            if not options.append:
                output.write(json.dumps(dict(event="environment", platform=platform.platform(),
                    architecture=platform.machine(), cpus=os.cpu_count(), threads=options.threads,
                    allocator=options.allocator if not options.binary else "selected by supplied binary",
                    toolchain=options.toolchain, steps=options.steps, samples=options.samples,
                    fixture_sha256=hashlib.sha256((HERE / "test-all-second.txt").read_bytes()).hexdigest(),
                    profile="release", timing="native algorithms; parsing, JSON, transport, and rendering excluded")) + "\n")
            for case, steps, fixture in cases:
                if options.case not in ["both", case]:
                    continue
                for sample in range(options.sample_start, options.sample_start + options.samples):
                    # Reverse implementation order on alternate samples.
                    modes = options.modes if sample % 2 else reversed(options.modes)
                    for mode in modes:
                        measure(binary, case, mode, steps, fixture, options.threads,
                            options.timeout, output, sample)


if __name__ == "__main__":
    main()
