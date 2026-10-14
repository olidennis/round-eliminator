#!/usr/bin/env python3
"""Compare pruning concurrency after the review optimizations, in a temporary copy."""

import argparse
import shutil
import subprocess
from pathlib import Path
from investigate_review import ROOT, prepare, replace


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--build", action="store_true")
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-target-linux"))
    options = parser.parse_args()
    directory = options.directory.resolve()
    prepare(directory)
    # Keep the driver/allocator manifest, then replace ALL algorithm sources
    # with the integrated production version. No earlier prototype hooks remain.
    shutil.copytree(ROOT / "crates/core/src", directory / "core/src", dirs_exist_ok=True)
    with (directory / "core/src/lib.rs").open("a") as out:
        out.write('\nmod review_flags;\n')
    (directory / "core/src/review_flags.rs").write_text('''
pub(crate) fn pruning_target() -> u128 {
    static TARGET: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    *TARGET.get_or_init(|| match std::env::var("RE_PROFILE_VARIANT").as_deref() {
        Ok("target10") => 10_000_000,
        Ok("target12") => 12_000_000,
        _ => 8_000_000,
    })
}
''')
    antichain = directory / "core/src/algorithms/maximize/antichain.rs"
    # Accept either integrated default; keep explicit historical 8ms baseline.
    source = antichain.read_text()
    old = ".div_ceil(12_000_000)" if ".div_ceil(12_000_000)" in source else ".div_ceil(8_000_000)"
    replace(antichain, old, ".div_ceil(crate::review_flags::pruning_target())")
    manifest = directory / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace('name = "re-review"', 'name = "re-review-policy"'))
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(manifest), "--features", "mimalloc-v2", "--target-dir", str(options.target_dir)], check=True)


if __name__ == "__main__":
    main()
