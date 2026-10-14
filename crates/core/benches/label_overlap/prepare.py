#!/usr/bin/env python3
"""Build an isolated before/after comparison of Boolean label overlap."""
import argparse
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
CORE = HERE.parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('destination', type=Path)
parser.add_argument('--build', action='store_true')
args = parser.parse_args()
destination = args.destination.resolve()
destination.mkdir(parents=True, exist_ok=True)
for variant in ['before', 'after']:
    target = destination / variant
    target.mkdir(exist_ok=True)
    shutil.copytree(CORE / 'src', target / 'src', dirs_exist_ok=True)
    manifest = (CORE / 'Cargo.toml').read_text().split('[[bench]]')[0]
    manifest = manifest.replace('name = "round-eliminator-3"', f'name = "overlap-{variant}"', 1)
    (target / 'Cargo.toml').write_text(manifest)
    if variant == 'before':
        path = target / 'src/algorithms/configurations.rs'
        source = path.read_text()
        old = '|offered, allowed| !offered.is_disjoint(allowed)'
        assert source.count(old) == 1
        path.write_text(source.replace(old, '|offered, allowed| !offered.intersection(allowed).is_empty()'))
(destination / 'Cargo.toml').write_text(f'''[package]
name = "label-overlap-comparison"
version = "0.0.0"
edition = "2024"
[workspace]
members = ["before", "after"]
[dependencies]
before = {{ package = "overlap-before", path = "before" }}
after = {{ package = "overlap-after", path = "after" }}
serde_json = "1"
mimalloc = {{ version = "=0.1.52", features = ["v2"] }}
libmimalloc-sys = {{ version = "=0.1.49", features = ["extended"] }}
petgraph = {{ version = "0.8.3", default-features = false, features = ["std"] }}
[[bin]]
name = "label-overlap-comparison"
path = "{HERE / 'native.rs'}"
''')
if args.build:
    subprocess.run(['cargo', '+1.89.0', 'build', '--release', '--manifest-path',
                    str(destination / 'Cargo.toml'), '--target-dir', '/tmp/re3-overlap-target'], check=True)
