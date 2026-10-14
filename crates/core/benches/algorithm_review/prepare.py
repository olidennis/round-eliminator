#!/usr/bin/env python3
"""Build isolated current/baseline kernels for the seven requested experiments."""
import argparse
from pathlib import Path
import shutil
import subprocess
HERE = Path(__file__).resolve().parent
CORE = HERE.parent.parent
p=argparse.ArgumentParser(description=__doc__);p.add_argument('destination', type=Path);p.add_argument('--build',action='store_true');p.add_argument('--only',choices=['enumerate','diagram','membership','degree_two','symmetry','lattice']);p.add_argument('--symmetry',choices=['tensor','profiles']);a=p.parse_args()
destination=a.destination.resolve();destination.mkdir(parents=True,exist_ok=True)
for variant in ['before','after']:
 target=destination/variant;target.mkdir(exist_ok=True)
 shutil.copytree(CORE/'src',target/'src',dirs_exist_ok=True)
 manifest=(CORE/'Cargo.toml').read_text().split('[[bench]]')[0].replace('name = "round-eliminator-3"',f'name = "seven-{variant}"',1)
 (target/'Cargo.toml').write_text(manifest)
 for name,relative in [('configurations.rs','algorithms/configurations.rs'),('diagram_relation.rs','algorithms/diagram/relation.rs'),('zero_round.rs','algorithms/solvability/zero_round/mod.rs'),('distance_two.rs','algorithms/solvability/coloring/distance_two.rs'),('lattice.rs','algorithms/fixed_point/lattice.rs')]:
  category={'configurations.rs':'enumerate','diagram_relation.rs':'diagram','zero_round.rs':'zero','distance_two.rs':'symmetry','lattice.rs':'lattice'}[name]
  selected=category==a.only or (category=='zero' and a.only in ['membership','degree_two'])
  if variant=='before' or (a.only and not selected):
   shutil.copyfile(HERE/'baseline'/name,target/'src'/relative)
 if variant=='after' and a.symmetry:
  shutil.copyfile(HERE/f'distance_two_{a.symmetry}.rs',target/'src/algorithms/solvability/coloring/distance_two.rs')
 if variant=='before' or (a.only and a.only!='lattice'):
  shutil.copyfile(HERE/'baseline/fixed_point.rs',target/'src/algorithms/fixed_point.rs')
 configs=target/'src/algorithms/configurations.rs'
 configs.write_text(configs.read_text()+'''\npub fn benchmark_visit(family: &crate::model::CondensedConfiguration) -> usize {
 let mut count=0;
 visit_multisets_with(family, &crate::computation::Cancellation::default(), |row, origins| {
  std::hint::black_box((row,origins)); count+=1; Ok(true)
 }).unwrap(); count
}\n''')
 zero=target/'src/algorithms/solvability/zero_round/mod.rs';s=zero.read_text()
 if variant=='after':
  s=s.replace('#[cfg(test)]\nmod membership;','mod membership;').replace('#[cfg(test)]\nmod degree_two;','mod degree_two;')
  for module in ['membership','degree_two']:
   if f'mod {module};' not in s: s+=f'\nmod {module};\n'
  shutil.copyfile(HERE/'membership.rs',target/'src/algorithms/solvability/zero_round/membership.rs')
  arms='''"membership" => membership::check(problem, PassiveMode::General, None, &mut computation),
 "degree_two" if degree_two::applicable(problem) => degree_two::check(problem,&mut computation),'''
 else: arms=''
 s+='''\npub fn benchmark_check(problem: &crate::model::Problem, algorithm: &str) -> anyhow::Result<ZeroRoundResult> {
 let mut report=|_| {};
 let mut computation=crate::computation::Computation::new(crate::computation::Cancellation::default(),&mut report);
 match algorithm {'''+arms+''' _=>Ok(search(problem,PassiveMode::General,None,crate::algorithms::sat::backend(&computation.cancellation),&mut computation)?) }
}\n''';zero.write_text(s)
 fixed=target/'src/algorithms/fixed_point.rs';s=fixed.read_text();s+='''
pub fn benchmark_construct(problem: &crate::model::Problem)->anyhow::Result<crate::model::Problem> {
 construct_with(problem,&mut crate::computation::Computation::new(crate::computation::Cancellation::default(),&mut |_|{}))
}

pub fn benchmark_lattice(bits: usize, operations: usize) -> (usize,u128,u128,u64) {
 let started=std::time::Instant::now();
 let mut report=|_| {};
 let lattice=lattice::Lattice::new(&vec![vec![];bits], &mut crate::computation::Computation::new(
 crate::computation::Cancellation::default(), &mut report)).unwrap();
 let preparation=started.elapsed().as_nanos(); let started=std::time::Instant::now();
 fn exercise<O: crate::algorithms::maximize::Operations<crate::model::LabelId>>(algebra:&O,size:usize,operations:usize)->u64 {
 let mut checksum=0u64;
 for i in 0..operations {
 let a=std::hint::black_box(crate::model::LabelId((i%size) as u32));
 let b=std::hint::black_box(crate::model::LabelId(((i*31+7)%size) as u32));
 checksum+=algebra.union(&a,&b).0 as u64 + algebra.intersection(&a,&b).0 as u64 + algebra.is_superset(&a,&b) as u64;
 } checksum }
 let checksum=exercise(&lattice::LatticeOperations {lattice:&lattice,dual:false},lattice.sets.len(),operations);
 (lattice.sets.len(), preparation, started.elapsed().as_nanos(), checksum)
}\n''';
 if variant=='after' and (not a.only or a.only=='lattice'):
  s=s.replace('let checksum=exercise(&lattice::LatticeOperations {lattice:&lattice,dual:false},lattice.sets.len(),operations);', 'let checksum=if let Some(ops)=lattice.table_operations(false){exercise(&ops,lattice.sets.len(),operations)}else{exercise(&lattice::LatticeOperations {lattice:&lattice,dual:false},lattice.sets.len(),operations)};')
 fixed.write_text(s)
(destination/'Cargo.toml').write_text(f'''[package]
name = "algorithm-review"
version = "0.0.0"
edition = "2024"
[workspace]
members=["before","after"]
[dependencies]
before={{package="seven-before",path="before"}}
after={{package="seven-after",path="after"}}
serde_json="1"
mimalloc={{version="=0.1.52",features=["v2"]}}
libmimalloc-sys={{version="=0.1.49",features=["extended"]}}
[[bin]]
name="algorithm-review"
path="{HERE/'native.rs'}"
''')
if a.build: subprocess.run(['cargo','+1.89.0','build','--release','--manifest-path',str(destination/'Cargo.toml'),'--target-dir','/tmp/re3-overlap-target'],check=True)
