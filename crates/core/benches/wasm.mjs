// Run the native benchmark's fixtures inside release wasm, without changing
// the application API or keeping a separate benchmark crate in the workspace.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const core = fileURLToPath(new URL('../', import.meta.url))
const fixtures = fileURLToPath(new URL('./support/matching.rs', import.meta.url))
const target = fileURLToPath(new URL('../../../target', import.meta.url))
const temporary = await mkdtemp(join(tmpdir(), 're3-matching-'))
try {
  await mkdir(join(temporary, 'src'))
  await writeFile(join(temporary, 'Cargo.toml'), `
[package]
name = "re3-matching-bench-wasm"
version = "0.0.0"
edition = "2024"
[workspace]
[lib]
crate-type = ["cdylib"]
[dependencies]
round-eliminator-3 = { path = ${JSON.stringify(core)} }
petgraph = { version = "0.8.3", default-features = false, features = ["std"] }
wasm-bindgen = "0.2.129"
`)
  await writeFile(join(temporary, 'src/lib.rs'), `
use wasm_bindgen::prelude::*;
#[path = ${JSON.stringify(fixtures)}]
mod matching;
#[wasm_bindgen]
pub struct Benchmark { cases: Vec<matching::Case> }
#[wasm_bindgen]
impl Benchmark {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self { Self { cases: matching::cases() } }
    pub fn case_name(&self, index: usize) -> String { self.cases[index].name.into() }
    pub fn case_count(&self) -> usize { self.cases.len() }
    pub fn algorithm_name(&self, index: usize) -> String { matching::ALGORITHMS[index].into() }
    pub fn check(&self, index: usize, algorithm: usize, iterations: usize) -> usize {
        let case = &self.cases[index];
        let mut correct = 0;
        for _ in 0..iterations {
            let actual = std::hint::black_box(matching::run(std::hint::black_box(case), algorithm));
            correct += usize::from(actual == case.expected);
        }
        correct
    }
}
`)
  execFileSync('wasm-pack', ['build', '--target', 'web', '--release', '--out-name', 'matching'], {
    cwd: temporary,
    env: { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? target },
    stdio: ['ignore', 2, 2],
  })
  const { default: init, Benchmark } = await import(pathToFileURL(join(temporary, 'pkg/matching.js')))
  await init({ module_or_path: await readFile(join(temporary, 'pkg/matching_bg.wasm')) })
  const benchmark = new Benchmark()
  console.log('case,algorithm,median_ns_per_check')
  for (let fixture = 0; fixture < benchmark.case_count(); fixture++) {
    for (let algorithm = 0; algorithm < 4; algorithm++) {
      // Calibrate outside the measured samples. Each batch stays entirely in
      // wasm, so JS calls and fixture construction do not dominate small checks.
      let iterations = 1
      while (true) {
        const start = performance.now()
        assert.equal(benchmark.check(fixture, algorithm, iterations), iterations)
        const elapsed = performance.now() - start
        if (elapsed >= 50) break
        iterations *= Math.max(2, Math.min(100, Math.ceil(60 / Math.max(elapsed, 0.001))))
      }
      const samples = []
      for (let sample = 0; sample < 3; sample++) {
        const start = performance.now()
        assert.equal(benchmark.check(fixture, algorithm, iterations), iterations)
        samples.push((performance.now() - start) * 1e6 / iterations)
      }
      samples.sort((a, b) => a - b)
      console.log(`"${benchmark.case_name(fixture)}",${benchmark.algorithm_name(algorithm)},${samples[1].toFixed(1)}`)
    }
  }
  benchmark.free()
} finally {
  await rm(temporary, { recursive: true, force: true })
}
