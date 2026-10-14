#!/usr/bin/env python3
"""Profile current kernels and controlled variants in temporary native copies.

All experiments use configured mimalloc v2. Production sources are untouched.
Counters are optional: use --counts for operation counts, otherwise phase timers
remain but per-operation hooks compile away. Flags use RE_PROFILE_VARIANT.
"""

import argparse
from pathlib import Path
import shutil
import subprocess

from investigate import HERE, ROOT, OLD, patch_source, replace


def prepare(directory, counts):
    directory.mkdir(parents=True, exist_ok=True)
    hooks = patch_source((HERE / "profile_hooks.rs").read_text())
    hooks = hooks.replace("const N: usize = 43;", "const N: usize = 47;")
    hooks = hooks.replace('    "refresh_wall_ns",', '''    "refresh_wall_ns",
    "generation_user_ticks", "generation_system_ticks",
    "merge_user_ticks", "merge_system_ticks",''')
    hooks = hooks.replace("    add(index, 1);", "    if cfg!(feature = \"work-counts\") { add(index, 1); }")
    hooks = hooks.replace("pub fn matched() {", "pub fn matched() {\n    if !cfg!(feature = \"work-counts\") { return; }")
    hooks = hooks.replace("fn flush() {", "pub fn flush() {")
    # The parallel dispatcher flushes once per job; old scoped threads flush on exit.
    hooks = hooks.replace("            flush();\n", "")
    hooks += '''
// Linux process CPU sums all threads. Ticks are reported separately from wall
// time; this VM uses 100 Hz. File reads happen only at batch phase boundaries.
fn cpu_ticks() -> (u64, u64) {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let fields: Vec<_> = stat.rsplit_once(") ").unwrap().1.split_whitespace().collect();
    (fields[11].parse().unwrap(), fields[12].parse().unwrap())
}
pub struct Stage { start: Instant, cpu: (u64,u64), wall: usize, user: usize }
pub fn stage(wall: usize, user: usize) -> Stage {
    Stage { start: Instant::now(), cpu: cpu_ticks(), wall, user }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let cpu = cpu_ticks();
        add(self.wall, self.start.elapsed().as_nanos() as u64);
        add(self.user, cpu.0.saturating_sub(self.cpu.0));
        add(self.user+1, cpu.1.saturating_sub(self.cpu.1));
    }
}
pub struct Flush;
impl Drop for Flush { fn drop(&mut self) { flush(); } }
'''
    for name, source in [("core", ROOT / "crates/core"), ("old", OLD)]:
        crate = directory / name
        shutil.copytree(source / "src", crate / "src", dirs_exist_ok=True)
        if name == "core":
            # Preserve the before-integration experiment after production evolves.
            baseline = HERE / "investigation/baseline/2026-10-13"
            shutil.copy(baseline / "pre-live/maximize.rs", crate / "src/algorithms/maximize.rs")
            shutil.copy(baseline / "review/algorithms/configurations.rs", crate / "src/algorithms/configurations.rs")
            shutil.copy(baseline / "review/model/labels.rs", crate / "src/model/labels.rs")
            for filename, target in {
                "engine.rs": "maximize/engine.rs",
                "combination.rs": "maximize/combination.rs",
                "antichain.rs": "maximize/antichain.rs",
                "parallel.rs": "parallel.rs",
                "alignments.rs": "configurations/alignments.rs",
                "matching.rs": "configurations/matching.rs",
            }.items():
                shutil.copy(baseline / filename, crate / "src/algorithms" / target)
            configurations = crate / "src/algorithms/configurations.rs"
            text = configurations.read_text().replace(
                "use alignments::{visit_alignments_avoiding_with, visit_alignments_with};",
                "use alignments::visit_alignments_with;",
            )
            configurations.write_text(text)
        if (source / "benches/support").is_dir():
            shutil.copytree(source / "benches/support", crate / "benches/support", dirs_exist_ok=True)
        manifest = (source / "Cargo.toml").read_text().split("[[bench]]")[0].split("[patch.crates-io]")[0]
        (crate / "Cargo.toml").write_text(manifest + "\n[features]\nwork-counts = []\n" if name == "core" else manifest.replace("[features]", "[features]\nwork-counts = []"))
        (crate / "src/profile_hooks.rs").write_text(hooks)
        with (crate / "src/lib.rs").open("a") as output:
            output.write("\npub mod profile_hooks;\n")
    shutil.copy(HERE / "main.rs", directory / "main.rs")
    shutil.copy(HERE / "fixed_point.rs", directory / "fixed_point.rs")
    main = directory / "main.rs"
    replace(main, '            let (maximum, milliseconds) = if mode == "resource-old" {', '''            round_eliminator_lib::profile_hooks::reset();
            round_eliminator_3::profile_hooks::reset();
            let (maximum, milliseconds) = if mode == "resource-old" {''')
    replace(main, '"milliseconds":milliseconds,"maxima":maximum.configurations().count()', '''"milliseconds":milliseconds,"maxima":maximum.configurations().count(),
                "diagnostics":if mode == "resource-old" { round_eliminator_lib::profile_hooks::snapshot() } else { round_eliminator_3::profile_hooks::snapshot() }''')
    fixed = directory / "fixed_point.rs"
    replace(fixed, "    result.maximize_custom(", "    round_eliminator_lib::profile_hooks::reset();\n    result.maximize_custom(")
    replace(fixed, "    (counted(&result), milliseconds)", '''    emit(serde_json::json!({"event":"diagnostics", "implementation":"old",
        "degree":result.degree, "counters":round_eliminator_lib::profile_hooks::snapshot()}));
    (counted(&result), milliseconds)''')
    replace(fixed, "    let result = maximize_custom_with(initial, algebra, &mut computation, None).unwrap();", '''    round_eliminator_3::profile_hooks::reset();
    let result = maximize_custom_with(initial, algebra, &mut computation, None).unwrap();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    emit(serde_json::json!({"event":"diagnostics", "implementation":"new",
        "degree":initial[0].degree(), "counters":round_eliminator_3::profile_hooks::snapshot()}));''')
    replace(fixed, "    (result, start.elapsed().as_secs_f64() * 1000.0)", "    (result, ms)")
    engine = directory / "core/src/algorithms/maximize/engine.rs"
    replace(engine, "    let tracking = observe.is_some();", '''    let batch_size = std::env::var("RE_PROFILE_BATCH").ok().map(|s| s.parse().unwrap()).unwrap_or(batch_size);
    let tracking = observe.is_some();''')
    replace(engine, "            let snapshot = current.as_slice().to_vec();", "            crate::profile_hooks::inc(14);\n            let snapshot = current.as_slice().to_vec();")
    replace(engine, "                let results = map_with_progress(\n                    pairs.len(),", '''                let generation = crate::profile_hooks::stage(20, 43);
                let chunk_size = std::env::var("RE_PROFILE_CHUNK").ok().map(|s| s.parse::<usize>().unwrap()).unwrap_or(1);
                let results = map_with_progress(
                    pairs.len().div_ceil(chunk_size),''')
    replace(engine, "                        let (a, b) = pairs[index];\n                        combine(", '''                        let mut combined = Vec::new();
                        for &(a,b) in &pairs[index*chunk_size..((index+1)*chunk_size).min(pairs.len())] {
                        combined.push(combine(''')
    replace(engine, "                            tracking,\n                        )", "                            tracking,\n                            &seen,\n                        )?);\n                        }\n                        Ok(combined)")
    replace(engine, "completed = completed.saturating_add(1);", "completed = completed.saturating_add(chunk_size as u32);")
    replace(engine, "                let fresh = new_candidates(results,", '''                drop(generation);
                let merge = crate::profile_hooks::stage(21, 45);
                let results = results.into_iter().flatten().collect();
                let fresh = new_candidates(results,''')
    replace(engine, "                pairs.clear();", "                drop(merge);\n                pairs.clear();")
    replace(engine, "    let mut fresh = Vec::new();", "    let _history = crate::profile_hooks::timer(30);\n    let mut fresh = Vec::new();")
    replace(engine, "            fresh.push(candidate);", "            fresh.push(candidate);\n        } else {\n            crate::profile_hooks::inc(6);")
    combination = directory / "core/src/algorithms/maximize/combination.rs"
    replace(combination, "    tracking: bool,\n)", "    tracking: bool,\n    known: &BTreeSet<Configuration<T>>,\n)")
    replace(combination, "    let a = left.multiplicities();", '''    #[cfg(feature="work-counts")]
    let _pair = crate::profile_hooks::combining();
    crate::profile_hooks::inc(0);
    let a = left.multiplicities();''')
    replace(combination, "    for (i, (x, _)) in a.iter().enumerate() {", '''    let mut choices = Vec::new();
    for (i, (x, _)) in a.iter().enumerate() {''')
    replace(combination, "            cancellation.check()?;\n            if operations.is_superset", "            crate::profile_hooks::inc(10);\n            cancellation.check()?;\n            if operations.is_superset")
    replace(combination, "            let mut rows: Vec<_>", '''            choices.push((i,j,union));
        }
    }
    if crate::profile_hooks::enabled("union-prune") {
        let original = choices;
        let mut by_union: std::collections::BTreeMap<T, Vec<(usize,usize)>> = std::collections::BTreeMap::new();
        for (i,j,union) in original {
            let same = by_union.entry(union).or_default();
            let len = same.len();
            same.retain(|&(x,y)| !(operations.is_superset(&a[x].0,&a[i].0) && operations.is_superset(&b[y].0,&b[j].0)));
            if len != same.len() || same.iter().all(|&(x,y)| !(operations.is_superset(&a[i].0,&a[x].0) && operations.is_superset(&b[j].0,&b[y].0))) {
                same.push((i,j));
            } else { crate::profile_hooks::inc(17); }
        }
        choices = by_union.into_iter().flat_map(|(union,positions)| positions.into_iter().map(move |(i,j)| (i,j,union.clone()))).collect();
    }
    for (i,j,union) in choices {
            crate::profile_hooks::inc(1);
            let mut rows: Vec<_>''')
    replace(combination, "                if conflicts", "                crate::profile_hooks::inc(2);\n                if conflicts")
    replace(combination, "                    return Ok(());\n                }\n                let mut parts", "                    crate::profile_hooks::inc(11);\n                    return Ok(());\n                }\n                crate::profile_hooks::inc(3);\n                let mut parts")
    replace(combination, "                if !seen.insert(configuration.clone()) {", '''                if crate::profile_hooks::enabled("early-known") && known.contains(&configuration) {
                    crate::profile_hooks::inc(5);
                    return Ok(());
                }
                if !seen.insert(configuration.clone()) {
                    crate::profile_hooks::inc(4);''')
    replace(combination, "        }\n    }\n    Ok(result)", "    }\n    crate::profile_hooks::add(12, result.len() as u64);\n    Ok(result)")
    # Optional earlier rejection of the same dominated exchanges: stop a matrix
    # branch once it occupies both cells, instead of waiting for a complete matrix.
    configurations = directory / "core/src/algorithms/configurations.rs"
    replace(configurations, "use alignments::visit_alignments_with;", "use alignments::{visit_alignments_with, visit_alignments_avoiding_with};")
    replace(combination, "configurations::visit_alignments_with", "configurations::visit_alignments_avoiding_with")
    replace(combination, "visit_alignments_with(&rows, &columns, &compatible, cancellation, |counts| {", '''visit_alignments_avoiding_with(&rows, &columns, &compatible,
                if crate::profile_hooks::enabled("conflict-search") { &conflicts } else { &[] },
                cancellation, |counts| {''')
    alignments = directory / "core/src/algorithms/configurations/alignments.rs"
    replace(alignments, "pub(crate) fn visit_alignments_with(", '''pub(crate) fn visit_alignments_with(
    rows: &[u32], columns: &[u32], compatible: &[bool], cancellation: &Cancellation,
    visitor: impl FnMut(&[u32]) -> Result<(), Cancelled>,
) -> Result<(), Cancelled> {
    visit_alignments_avoiding_with(rows, columns, compatible, &[], cancellation, visitor)
}

pub(crate) fn visit_alignments_avoiding_with(''')
    replace(alignments, "    compatible: &[bool],\n    cancellation:", "    compatible: &[bool],\n    conflicts: &[(usize,usize)],\n    cancellation:")
    replace(alignments, "    let mut stack = Vec::new();", '''    let mut forbidden = if conflicts.is_empty() { Vec::new() } else { vec![Vec::new(); compatible.len()] };
    for &(x,y) in conflicts { forbidden[x.max(y)].push(x.min(y)); }
    let mut stack = Vec::new();''')
    replace(alignments, "        frame.chosen = Some(count);", '''        frame.chosen = Some(count);
        if count > 0 && !forbidden.is_empty() && forbidden[frame.cell].iter().any(|&other| search.counts[other] > 0) {
            continue;
        }''')
    with alignments.open("a") as output:
        output.write('''
#[cfg(test)]
mod conflict_oracle_tests {
    use super::*;
    #[test]
    fn early_conflicts_equal_filtered_complete_matrices() {
        let cancellation = Cancellation::default();
        let cell_pairs: Vec<_> = (0..4).flat_map(|a| (a+1..4).map(move |b| (a,b))).collect();
        for a in 0..=2 { for b in 0..=2 { for c in 0..=2 { for d in 0..=2 {
            if a+b != c+d { continue; }
            for allowed_mask in 0u32..16 {
                let allowed: Vec<_> = (0..4).map(|i| allowed_mask & (1<<i) != 0).collect();
                let mut complete = Vec::new();
                visit_alignments_with(&[a,b], &[c,d], &allowed, &cancellation, |m| {
                    complete.push(m.to_vec()); Ok(())
                }).unwrap();
                for conflict_mask in 0u32..64 {
                    let conflicts: Vec<_> = cell_pairs.iter().copied().enumerate()
                        .filter_map(|(i,pair)| (conflict_mask & (1<<i) != 0).then_some(pair)).collect();
                    let expected: Vec<_> = complete.iter().filter(|m|
                        conflicts.iter().all(|&(x,y)| m[x] == 0 || m[y] == 0)).cloned().collect();
                    let mut actual = Vec::new();
                    visit_alignments_avoiding_with(&[a,b], &[c,d], &allowed, &conflicts, &cancellation, |m| {
                        actual.push(m.to_vec()); Ok(())
                    }).unwrap();
                    assert_eq!(actual, expected);
                }
            }
        }}}}
    }
}
''')
    antichain = directory / "core/src/algorithms/maximize/antichain.rs"
    replace(antichain, "        let parallel = can_parallelize()", "        let parallel = !crate::profile_hooks::enabled(\"serial-pruning\") && can_parallelize()")
    replace(antichain, "        for old in &self.recent {", "        for old in &self.recent {\n            crate::profile_hooks::inc(33);")
    replace(antichain, "        for old in &self.configurations {", "        for old in &self.configurations {\n            crate::profile_hooks::inc(35);")
    replace(antichain, "        for old in self.configurations.drain(..) {", "        for old in self.configurations.drain(..) {\n            crate::profile_hooks::inc(37);")
    replace(antichain, "                    for old in cache.iter() {", "                    for old in cache.iter() {\n                        crate::profile_hooks::inc(33);")
    replace(antichain, "                        for old in current {", "                        for old in current {\n                            crate::profile_hooks::inc(35);")
    replace(antichain, "                        for candidate in &candidates {", "                        for candidate in &candidates {\n                            crate::profile_hooks::inc(37);")
    replace(antichain, "                            if other != own", "                            if other != own { crate::profile_hooks::inc(38); }\n                            if other != own")
    replace(antichain, "        let current = &self.configurations;\n        let count", '''        let peers_first = crate::profile_hooks::enabled("peers-first");
        let candidates = if peers_first && candidates.len() > 1 {
            let size = candidates.len().div_ceil(worker_count());
            let masks = map_with_progress(candidates.len().div_ceil(size), cancellation, true, |chunk| {
                let mut removed = Vec::new();
                for own in chunk*size..((chunk+1)*size).min(candidates.len()) {
                    let mut dominated = false;
                    for (other,candidate) in candidates.iter().enumerate() {
                        cancellation.check()?;
                        if other != own {
                            crate::profile_hooks::inc(38);
                            if dominates(&candidate.configuration, &candidates[own].configuration, operations, cancellation)? {
                                dominated = true; break;
                            }
                        }
                    }
                    removed.push(dominated);
                }
                Ok(removed)
            }, |_| cancellation.check())?;
            candidates.into_iter().zip(masks.into_iter().flatten())
                .filter_map(|(candidate,removed)| (!removed).then_some(candidate)).collect()
        } else { candidates };
        let current = &self.configurations;
        let count''')
    replace(antichain, "                        let own = index - current.len();", '''                        if peers_first { removed.push(false); continue; }
                        let own = index - current.len();''')
    matching = directory / "core/src/algorithms/configurations/matching.rs"
    replace(matching, "    cancellation.check()?;\n    let degree", "    crate::profile_hooks::matched();\n    cancellation.check()?;\n    let degree")
    replace(matching, "    let matched = dinics", "    crate::profile_hooks::inc(9);\n    let matched = dinics")
    parallel = directory / "core/src/algorithms/parallel.rs"
    replace(parallel, "                    let result = cancellation.check()", '''                    #[cfg(feature="work-counts")]
                    let _flush = crate::profile_hooks::Flush;
                    let result = cancellation.check()''')
    old = directory / "old/src/algorithms/maximize.rs"
    replace(old, "    let unions = good_unions(l1, l2, f_is_superset, f_union);", '''    #[cfg(feature="work-counts")]
    let _pair = crate::profile_hooks::combining();
    crate::profile_hooks::inc(0);
    let unions = good_unions(l1, l2, f_is_superset, f_union);
    crate::profile_hooks::add(1, unions.values().map(Vec::len).sum::<usize>() as u64);''')
    replace(old, "'outer: while let Some(pairing) = pairings.next() {", "'outer: while let Some(pairing) = pairings.next() {\n        crate::profile_hooks::inc(2);")
    replace(old, "        result.push(parts);", "        crate::profile_hooks::inc(16);\n        result.push(parts);")
    replace(old, "                let compressed = newline.compressed();", "                crate::profile_hooks::inc(3);\n                let compressed = newline.compressed();")
    replace(old, "                if !seen.contains(&compressed) {", "                let known = seen.contains(&compressed);\n                if known { crate::profile_hooks::inc(5); }\n                if !known {")
    replace(old, "    (result.lines,line_to_unions,line_to_allinfo)", "    crate::profile_hooks::add(12,result.lines.len() as u64);\n    (result.lines,line_to_unions,line_to_allinfo)")
    inclusion = directory / "old/src/algorithms/line_inclusion.rs"
    replace(inclusion, "        let d1 = self.parts.len();", "        crate::profile_hooks::matched();\n        let d1 = self.parts.len();")
    replace(inclusion, "        let (flowvalue, flow) = g.dinic", "        crate::profile_hooks::inc(9);\n        let (flowvalue, flow) = g.dinic")
    shutil.copy(HERE / "Cargo.lock", directory / "Cargo.lock")
    name = "re-current-counts" if counts else "re-current-phases"
    features = ', features = ["work-counts"]' if counts else ''
    old_features = ', "work-counts"' if counts else ''
    (directory / "Cargo.toml").write_text(f'''[package]
name = "{name}"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
round-eliminator-3 = {{ path = "core"{features} }}
round-eliminator-lib = {{ path = "old", default-features = false, features = ["onlyrust"{old_features}] }}
serde_json = "1"
rayon = "1"
mimalloc = {{ version = "=0.1.52", features = ["v2"] }}
libmimalloc-sys = {{ version = "=0.1.49", features = ["extended"] }}
[features]
default = ["mimalloc"]
mimalloc = []
jemalloc = []
[[bin]]
name = "{name}"
path = "main.rs"
''')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--counts", action="store_true")
    parser.add_argument("--build", action="store_true")
    parser.add_argument("--target-dir", type=Path, default=Path("/tmp/re3-target-linux"))
    options = parser.parse_args()
    prepare(options.directory.resolve(), options.counts)
    if options.build:
        subprocess.run(["cargo", "+1.89.0", "build", "--release", "--manifest-path",
            str(options.directory.resolve()/"Cargo.toml"), "--target-dir", str(options.target_dir)], check=True)


if __name__ == "__main__":
    main()
