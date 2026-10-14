#!/usr/bin/env python3
"""Build a native diagnostic executable without adding workspace dependencies."""
import argparse
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
CORE = HERE.parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--package', type=Path, default=Path('/tmp/re3-coloring-probe'))
parser.add_argument('--core', type=Path, default=CORE)
parser.add_argument('--target', type=Path, default=Path('/tmp/re3-target-linux'))
args = parser.parse_args()
args.package.mkdir(parents=True, exist_ok=True)
(args.package / 'Cargo.toml').write_text(f'''[package]
name = "re3-coloring-probe"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
round-eliminator-3 = {{ path = {json.dumps(str(args.core.resolve()))} }}
serde_json = "1"
mimalloc = {{ version = "=0.1.52", features = ["v2"] }}
libmimalloc-sys = {{ version = "=0.1.49", features = ["extended"] }}
[[bin]]
name = "re3-coloring-probe"
path = {json.dumps(str(HERE / 'native.rs'))}
''')
subprocess.run(['cargo', '+1.89.0', 'build', '--release', '--manifest-path',
                str(args.package / 'Cargo.toml'), '--target-dir', str(args.target)], check=True)
print(args.target / 'release/re3-coloring-probe')
