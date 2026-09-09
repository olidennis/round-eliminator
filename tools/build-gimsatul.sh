#!/bin/sh
# Optional native diagram solver, pinned to the revision benchmarked here.
set -eu
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tools_dir="$repo_dir/target/native-tools"
mkdir -p "$tools_dir"
source_dir=$(mktemp -d "$tools_dir/gimsatul-src.XXXXXXXX")
git clone "${RE_GIMSATUL_SOURCE:-https://github.com/arminbiere/gimsatul.git}" "$source_dir"
git -C "$source_dir" checkout --detach 4664fd74c97f87e30e7f907181707679b6fa49f2
cd "$source_dir"
./configure
make -j "${RE_BUILD_THREADS:-2}"
cp gimsatul "$tools_dir/gimsatul"
cp LICENSE "$tools_dir/gimsatul.LICENSE"
printf 'Built native solver: %s\nSource retained at: %s\n' "$tools_dir/gimsatul" "$source_dir"
