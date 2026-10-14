#!/usr/bin/env python3
"""Instrument temporary copies; compare controlled changes against fixed inputs."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
OLD = ROOT.parent / "round-eliminator" / "round-eliminator-lib"


def replace(path, before, after):
    text = path.read_text()
    if text.count(before) != 1:
        raise RuntimeError(f"Expected exactly one profiling anchor in {path}: {before[:60]}")
    path.write_text(text.replace(before, after))


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=True)
    for name, source in [("core", ROOT / "crates/core"), ("old", OLD)]:
        crate = directory / name
        shutil.copytree(source / "src", crate / "src", dirs_exist_ok=True)
        if name == "core":
            # Archive just the two original kernels so historical prototypes
            # remain reproducible after production optimization and splitting.
            baseline = HERE / "investigation/baseline"
            shutil.copy(baseline / "2026-10-13/pre-live/maximize.rs", crate / "src/algorithms/maximize.rs")
            shutil.copy(baseline / "2026-10-13/review/algorithms/configurations.rs", crate / "src/algorithms/configurations.rs")
            shutil.copy(baseline / "2026-10-13/review/algorithms/configurations/alignments.rs", crate / "src/algorithms/configurations/alignments.rs")
            shutil.copy(baseline / "2026-10-13/review/model/labels.rs", crate / "src/model/labels.rs")
            shutil.copy(baseline / "engine.rs", crate / "src/algorithms/maximize/engine.rs")
            shutil.copy(baseline / "matching.rs", crate / "src/algorithms/configurations/matching.rs")
            shutil.copy(baseline / "2026-10-13/parallel.rs", crate / "src/algorithms/parallel.rs")
            maximum = crate / "src/algorithms/maximize.rs"
            text = maximum.read_text()
            for declaration in ["mod antichain;\n", "mod combination;\n"]:
                text = text.replace(declaration, "")
            maximum.write_text(text)
        if (source / "benches/support").is_dir():
            shutil.copytree(source / "benches/support", crate / "benches/support", dirs_exist_ok=True)
        manifest = (source / "Cargo.toml").read_text()
        manifest = manifest.split("[[bench]]")[0].split("[patch.crates-io]")[0]
        (crate / "Cargo.toml").write_text(manifest)
        shutil.copy(HERE / "profile_hooks.rs", crate / "src/profile_hooks.rs")
        with (crate / "src/lib.rs").open("a") as lib:
            lib.write("\npub mod profile_hooks;\n")
    main = directory / "main.rs"
    shutil.copy(HERE / "main.rs", main)
    shutil.copy(HERE / "fixed_point.rs", directory / "fixed_point.rs")
    replace(main, "let start = Instant::now();\n            let count = if mode", "round_eliminator_lib::profile_hooks::reset();\n            round_eliminator_3::profile_hooks::reset();\n            let start = Instant::now();\n            let count = if mode")
    replace(main, '"milliseconds":start.elapsed().as_secs_f64()*1000.0,"maxima":count', '"milliseconds":start.elapsed().as_secs_f64()*1000.0,"maxima":count,\n                "diagnostics":if mode == "profile-old" { round_eliminator_lib::profile_hooks::snapshot() } else { round_eliminator_3::profile_hooks::snapshot() }')
    # Optional result dumping is included in diagnostic timing. Prototypes must
    # preserve the full canonical maximum, not merely the number of rows.
    replace(main, "std::hint::black_box(\n                    maximize_constraint_with(new.output().passive(), &mut computation).unwrap(),\n                )\n                .configurations()\n                .count()", "{ let maximum = maximize_constraint_with(new.output().passive(), &mut computation).unwrap();\n                    if let Ok(path) = std::env::var(\"RE_PROFILE_RESULT\") { std::fs::write(path, serde_json::to_vec(&maximum).unwrap()).unwrap(); }\n                    std::hint::black_box(maximum).configurations().count() }")
    engine = directory / "core/src/algorithms/maximize/engine.rs"
    replace(engine, "let batch_size = rayon::current_num_threads().saturating_mul(2).max(1);", "let batch_size = if crate::profile_hooks::enabled(\"batch-4096\") { 4096 } else if crate::profile_hooks::enabled(\"batch-1024\") { 1024 } else if crate::profile_hooks::enabled(\"batch-256\") { 256 } else { rayon::current_num_threads().saturating_mul(2).max(1) };")
    replace(engine, "let snapshot = current.clone();", "crate::profile_hooks::inc(14);\n            let snapshot = current.clone();")
    replace(engine, "let mut previous = BTreeSet::new();", "let generated = std::sync::Mutex::new(current.iter().cloned().collect::<BTreeSet<_>>());\n        let mut previous = BTreeSet::new();")
    replace(engine, "let results = map_with_progress(", "crate::profile_hooks::inc(if work >= 20_000 { 18 } else { 19 });\n                let generation = crate::profile_hooks::timer(20);\n                let results = map_with_progress(")
    replace(engine, "work >= 20_000,", "work >= 20_000 && !crate::profile_hooks::enabled(\"serial\"),")
    replace(engine, "tracking,\n                        )", "tracking,\n                            &seen,\n                            &generated,\n                        )")
    replace(engine, "for candidates in results {", "drop(generation);\n                let merge = crate::profile_hooks::timer(21);\n                for candidates in results {")
    replace(engine, "if !seen.insert(candidate.configuration.clone()) {\n                            continue;", "if !seen.insert(candidate.configuration.clone()) {\n                            crate::profile_hooks::inc(6);\n                            continue;")
    replace(engine, "changed = true;", "crate::profile_hooks::inc(13);\n                            changed = true;")
    replace(engine, "pairs.clear();", "drop(merge);\n                pairs.clear();")
    replace(engine, "tracking: bool,\n) ->", "tracking: bool,\n    known: &BTreeSet<Configuration<T>>,\n    generated: &std::sync::Mutex<BTreeSet<Configuration<T>>>,\n) ->")
    replace(engine, "let a = left.multiplicities();", "let _pair = crate::profile_hooks::combining();\n    crate::profile_hooks::inc(0);\n    let a = left.multiplicities();")
    replace(engine, "for (j, (y, _)) in b.iter().enumerate() {", "for (j, (y, _)) in b.iter().enumerate() {\n            crate::profile_hooks::inc(10);")
    replace(engine, "let mut rows: Vec<_>", "crate::profile_hooks::inc(1);\n            let mut rows: Vec<_>")
    replace(engine, "for old in current.iter() {", '''if crate::profile_hooks::enabled("recent") {
        for old in recent.iter() { if dominates(old,&candidate,operations,cancellation)? { return Ok(false); } }
    }
    let reverse = crate::profile_hooks::enabled("reverse");
    for index in 0..current.len() {
        let old = &current[if reverse { current.len()-1-index } else { index }];''')
    replace(engine, "if dominates(old, &candidate, operations, cancellation)? {\n            return Ok(false);", "if dominates(old, &candidate, operations, cancellation)? {\n            if crate::profile_hooks::enabled(\"recent\") { recent.push_front(old.clone()); recent.truncate((current.len()/25).max(1)); }\n            return Ok(false);")
    replace(engine, "fn insert<T: Clone + Ord>(\n    current: &mut Vec<Configuration<T>>,", "fn insert<T: Clone + Ord>(\n    current: &mut Vec<Configuration<T>>,\n    recent: &mut std::collections::VecDeque<Configuration<T>>,")
    replace(engine, "if crate::profile_hooks::enabled(\"recent\") {\n        for old", "let _commit = crate::profile_hooks::timer(29);\n    if crate::profile_hooks::enabled(\"recent\") {\n        for old")
    replace(engine, "let mut current = Vec::new();", "let mut current = Vec::new();\n        let mut recent = std::collections::VecDeque::new();")
    replace(engine, "insert(&mut current, seed, operations, &cancellation)?;", "insert(&mut current, &mut recent, seed, operations, &cancellation)?;")
    replace(engine, "&mut current,\n                            candidate.configuration.clone(),", "&mut current,\n                            &mut recent,\n                            candidate.configuration.clone(),")
    replace(engine, "let mut parts = vec![(union.clone(), 1)];", "crate::profile_hooks::inc(2);\n                if conflicts.iter().any(|&(x,y)| counts[x] > 0 && counts[y] > 0) { crate::profile_hooks::inc(11); return Ok(()); }\n                let mut parts = vec![(union.clone(), 1)];")
    replace(engine, 'let configuration =\n                    Configuration::new(parts).expect("combination preserves degree");', 'crate::profile_hooks::inc(3);\n                let configuration =\n                    Configuration::new(parts).expect("combination preserves degree");\n                if crate::profile_hooks::enabled("early-known") && known.contains(&configuration) { crate::profile_hooks::inc(5); return Ok(()); }')
    replace(engine, "if !seen.insert(configuration.clone()) {", "if crate::profile_hooks::enabled(\"shared\") && !generated.lock().unwrap().insert(configuration.clone()) { crate::profile_hooks::inc(15); return Ok(()); }\n                if !seen.insert(configuration.clone()) {")
    replace(engine, "if !seen.insert(configuration.clone()) {\n                    return", "if !seen.insert(configuration.clone()) {\n                    crate::profile_hooks::inc(4);\n                    return")
    replace(engine, "Ok(result)\n}", "crate::profile_hooks::add(12, result.len() as u64);\n    Ok(result)\n}")
    # An exchange of two occupied matrix cells dominates an alignment when its
    # two crossing intersections contain the original intersections, at least
    # one strictly. Precompute these conflicts once per parent pair.
    replace(engine, "let mut result: Vec<Candidate<T>> = Vec::new();", '''let mut conflicts = Vec::new();
    if crate::profile_hooks::enabled("swap") {
        for i in 0..a.len() { for k in i+1..a.len() {
            for j in 0..b.len() { for l in 0..b.len() {
                if j == l { continue; }
                let (x,y,u,v) = (i*b.len()+j,k*b.len()+l,i*b.len()+l,k*b.len()+j);
                if !compatible[x] || !compatible[y] { continue; }
                let (px,py,pu,pv) = (&intersections[x],&intersections[y],&intersections[u],&intersections[v]);
                if (operations.is_superset(pv,px) && operations.is_superset(pu,py) && (pv!=px || pu!=py))
                    || (operations.is_superset(pu,px) && operations.is_superset(pv,py) && (pu!=px || pv!=py)) {
                    conflicts.push((x,y));
                }
            }}
        }}
    }
    let mut result: Vec<Candidate<T>> = Vec::new();''')
    # Both variants read an immutable antichain. Traced runs retain ordered
    # insertion; untraced runs may also compute removals in parallel.
    shutil.copy(HERE / "parallel_pruning.rs", engine.parent / "parallel_pruning.rs")
    with engine.open("a") as source:
        source.write('\ninclude!("parallel_pruning.rs");\n')
    replace(engine, "let mut recent = std::collections::VecDeque::new();", "let mut recent = std::collections::VecDeque::new();\n        let mut pruning_caches = Vec::new();")
    replace(engine, "let merge = crate::profile_hooks::timer(21);\n                for candidates in results {", '''let merge = crate::profile_hooks::timer(21);
                if crate::profile_hooks::enabled("parallel-antichain") && !tracking {
                    changed |= parallel_merge(results, &mut current, &recent, &mut pruning_caches, &mut seen, operations, &cancellation)?;
                } else {
                let parallel_global = crate::profile_hooks::enabled("parallel-global") || crate::profile_hooks::enabled("parallel-antichain");
                let results = if parallel_global {
                    parallel_prune(results, &current, &recent, &mut pruning_caches, &mut seen, operations, &cancellation)?.0
                } else { results };
                for candidates in results {''')
    replace(engine, "if !seen.insert(candidate.configuration.clone()) {", "if !parallel_global && !seen.insert(candidate.configuration.clone()) {")
    replace(engine, "drop(merge);", "}\n                drop(merge);")
    # Separate serial cache/rejection/removal counts from the parallel passes.
    replace(engine, "for old in recent.iter() { if dominates(old,&candidate,operations,cancellation)? { return Ok(false); } }", "for old in recent.iter() { crate::profile_hooks::inc(33); if dominates(old,&candidate,operations,cancellation)? { crate::profile_hooks::inc(34); return Ok(false); } }")
    replace(engine, "let old = &current[if reverse { current.len()-1-index } else { index }];", "let old = &current[if reverse { current.len()-1-index } else { index }];\n        crate::profile_hooks::inc(35);")
    replace(engine, "if dominates(old, &candidate, operations, cancellation)? {\n            if", "if dominates(old, &candidate, operations, cancellation)? {\n            crate::profile_hooks::inc(36);\n            if")
    replace(engine, "if !dominates(&candidate, &old, operations, cancellation)? {", "crate::profile_hooks::inc(37);\n        if !dominates(&candidate, &old, operations, cancellation)? {")
    matching = directory / "core/src/algorithms/configurations/matching.rs"
    replace(matching, "cancellation.check()?;\n    let degree", "crate::profile_hooks::matched();\n    cancellation.check()?;\n    let degree")
    replace(matching, "let mut network = Graph::<(), u32>::new();", "let mut network = if crate::profile_hooks::enabled(\"preallocate\") { Graph::<(),u32>::with_capacity(2+offered.len()+allowed.len(), offered.len()+allowed.len()+offered.len()*allowed.len()) } else { Graph::<(),u32>::new() };")
    replace(matching, "let mut network = if", '''if crate::profile_hooks::enabled("capacity-first") {
        for (value,count) in offered {
            cancellation.check()?;
            let capacity: u64 = allowed.iter().filter(|(target,_)| compatible(value,target)).map(|(_,n)| u64::from(*n)).sum();
            if capacity < u64::from(*count) { return Ok(false); }
        }
    }
    let mut network = if''')
    replace(matching, "let matched = dinics", "crate::profile_hooks::inc(9);\n    let matched = dinics")
    # Old instrumentation leaves the original pruning and concurrency intact.
    old = directory / "old/src/algorithms/maximize.rs"
    replace(old, "let unions = good_unions(l1, l2, f_is_superset, f_union);", "let _pair = crate::profile_hooks::combining();\n    crate::profile_hooks::inc(0);\n    let unions = good_unions(l1, l2, f_is_superset, f_union);\n    crate::profile_hooks::add(1, unions.values().map(Vec::len).sum::<usize>() as u64);")
    replace(old, "'outer: while let Some(pairing) = pairings.next() {", "'outer: while let Some(pairing) = pairings.next() {\n        crate::profile_hooks::inc(2);")
    replace(old, "result.push(parts);", "crate::profile_hooks::inc(16);\n        result.push(parts);")
    replace(old, "let compressed = newline.compressed();", "crate::profile_hooks::inc(3);\n                let compressed = newline.compressed();")
    replace(old, "if !seen.contains(&compressed) {", "let known = seen.contains(&compressed);\n                if known { crate::profile_hooks::inc(15); }\n                if !known {")
    replace(old, "(result.lines,line_to_unions,line_to_allinfo)", "crate::profile_hooks::add(12, result.lines.len() as u64);\n    (result.lines,line_to_unions,line_to_allinfo)")
    inclusion = directory / "old/src/algorithms/line_inclusion.rs"
    replace(inclusion, "let d1 = self.parts.len();", "crate::profile_hooks::matched();\n        let d1 = self.parts.len();")
    replace(inclusion, "let (flowvalue, flow) = g.dinic", "crate::profile_hooks::inc(9);\n        let (flowvalue, flow) = g.dinic")
    (directory / "Cargo.lock").write_bytes((HERE / "Cargo.lock").read_bytes())
    (directory / "Cargo.toml").write_text(f'''[package]
name = "re-investigation"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
round-eliminator-3 = {{ path = "core" }}
round-eliminator-lib = {{ path = "old", default-features = false, features = ["onlyrust"] }}
serde_json = "1"
rayon = "1"
[[bin]]
name = "re-investigation"
path = "main.rs"
''')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--build", action="store_true")
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-target-linux"))
    options = parser.parse_args()
    prepare(options.directory.resolve())
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(options.directory.resolve()/"Cargo.toml"), "--target-dir", str(options.target_dir)], check=True)


if __name__ == "__main__":
    main()
