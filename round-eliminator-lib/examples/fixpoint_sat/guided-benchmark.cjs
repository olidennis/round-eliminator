// Benchmark the actual native GUI Loop path, with no known-certificate input.
// node guided-benchmark.cjs EXECUTABLE OUTPUT_DIRECTORY [SECONDS=180]
const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { createHash } = require('node:crypto');

async function main() {
  const [binary, directory, limit = '180'] = process.argv.slice(2);
  if (!binary || !directory || !/^[1-9]\d*$/.test(limit)) {
    throw new Error('Expected EXECUTABLE OUTPUT_DIRECTORY [SECONDS]');
  }
  const executable = path.resolve(binary);
  const root = path.resolve(directory);
  for (const suffix of ['stdout', 'stderr', 'time', 'json']) {
    const file = path.join(root, `loop.${suffix}`);
    if (fs.existsSync(file)) throw new Error(`Refusing to overwrite ${file}`);
  }
  const executableHash = createHash('sha256').update(fs.readFileSync(executable)).digest('hex');
  const out = fs.openSync(path.join(root, 'loop.stdout'), 'wx');
  const err = fs.openSync(path.join(root, 'loop.stderr'), 'wx');
  const args = ['-f', '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"peak_rss_kib":%M,"exit":%x}',
    '-o', path.join(root, 'loop.time'), 'timeout', '--signal=INT', '--kill-after=5s', `${limit}s`,
    executable, path.join(__dirname, 'hard_nonexistence.txt'), '--parallel'];
  console.log(`Starting native Loop, ${limit}s limit; logs: ${root}`);
  const result = await new Promise((resolve, reject) => {
    const child = spawn('/usr/bin/time', args, {
      env: { ...process.env, RE_NUM_THREADS: '1' }, stdio: ['ignore', out, err],
    });
    fs.closeSync(out);
    fs.closeSync(err);
    child.on('error', reject);
    child.on('close', (code, signal) => resolve({ code, signal }));
  });
  const output = fs.readFileSync(path.join(root, 'loop.stdout'), 'utf8');
  const progress = fs.readFileSync(path.join(root, 'loop.stderr'), 'utf8');
  const timing = fs.readFileSync(path.join(root, 'loop.time'), 'utf8').split('\n').find(l => l.startsWith('{'));
  const summary = {
    ...result, ...JSON.parse(timing), executable, executable_sha256: executableHash,
    limit_seconds: Number(limit),
    result: output.includes('No fixed point can be found.') ? 'VERIFIED_NONEXISTENCE'
      : result.code === 124 ? 'TIMEOUT' : 'OTHER',
    known_certificate_supplied: false,
    default_seed_starts: [...progress.matchAll(/Proof: default diagram seed starting:/g)].length,
    default_seed_nodes: [...progress.matchAll(/Proof: saturating default diagram: (\d+)/g)].map(m => Number(m[1])),
    default_seed_retained: [...progress.matchAll(/Proof: default diagram fragments retained: (\d+)/g)].map(m => Number(m[1])),
    guided_workers: [...progress.matchAll(/Proof: guided workers: (\d+)/g)].map(m => Number(m[1])),
    guided_peak_busy: [...progress.matchAll(/Proof: guided peak busy workers: (\d+)/g)].map(m => Number(m[1])),
    guided_jobs_completed: [...progress.matchAll(/Proof: guided jobs completed: (\d+)/g)].map(m => Number(m[1])),
    guided_variable_budget: [...progress.matchAll(/Proof: guided shared variable budget: (\d+)/g)].map(m => Number(m[1])),
    guided_bridge_jobs: [...progress.matchAll(/Proof: guided fresh\/hot bridge jobs: (\d+)\/(\d+)/g)]
      .map(m => ({ fresh: Number(m[1]), hot_retries: Number(m[2]) })),
    guided_default_used: [...progress.matchAll(/Proof: guided default fragments used: (\d+)\/(\d+)/g)]
      .map(m => ({ used: Number(m[1]), total: Number(m[2]) })),
    guided_new_used: [...progress.matchAll(/Proof: guided new fragments used: (\d+)/g)].map(m => Number(m[1])),
    guided_rotations: [...progress.matchAll(/Proof: guided archive rotations: (\d+)/g)].map(m => Number(m[1])),
    guided_pending_blocks: [...progress.matchAll(/Proof: guided pending changed blocks: (\d+)/g)].map(m => Number(m[1])),
    guided_cached: [...progress.matchAll(/Proof: guided cached jobs\/variables: (\d+)\/(\d+)/g)]
      .map(m => ({ jobs: Number(m[1]), variables: Number(m[2]) })),
    guided_variables: [...progress.matchAll(/guided SAT \((\d+) variables\)/g)].map(m => Number(m[1])),
    fragments: [...progress.matchAll(/guided (?:replayed game|retained derivation) fragments: (\d+)/g)].map(m => Number(m[1])),
    feedback_variables: [...progress.matchAll(/repair\/feedback SAT \((\d+) variables\)/g)].map(m => Number(m[1])),
    feedback_scores: [...progress.matchAll(/feedback pool \(best (\d+)\/(\d+)\): (\d+)\/(\d+)/g)]
      .map(m => ({ compatible_pairs: Number(m[1]), required_pairs: Number(m[2]),
        admitted: Number(m[3]), retained: Number(m[4]) })),
    repair_jobs: [...progress.matchAll(/Proof: repairing internal branches:/g)].length,
    cached_feedback_retries: [...progress.matchAll(/Proof: retrying cached repair\/feedback:/g)].length,
  };
  fs.writeFileSync(path.join(root, 'loop.json'), JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ ...summary, guided_variables: summary.guided_variables.length,
    guided_bridge_jobs: summary.guided_bridge_jobs.at(-1) || null,
    guided_default_used: summary.guided_default_used.at(-1) || null,
    guided_new_used: summary.guided_new_used.at(-1) || 0,
    guided_rotations: summary.guided_rotations.at(-1) || 0,
    guided_pending_blocks: summary.guided_pending_blocks.at(-1) || 0,
    guided_cached: summary.guided_cached.at(-1) || null,
    fragments: summary.fragments.at(-1) || 0,
    feedback_variables: summary.feedback_variables.length,
    feedback_scores: summary.feedback_scores.at(-1) || null }, null, 2));
  if (result.code !== 0 && result.code !== 124) process.exitCode = 1;
}
main().catch(error => { console.error(error); process.exitCode = 1; });
