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
    guided_variables: [...progress.matchAll(/guided SAT \((\d+) variables\)/g)].map(m => Number(m[1])),
    fragments: [...progress.matchAll(/guided replayed game fragments: (\d+)/g)].map(m => Number(m[1])),
  };
  fs.writeFileSync(path.join(root, 'loop.json'), JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ ...summary, guided_variables: summary.guided_variables.length,
    fragments: summary.fragments.at(-1) || 0 }, null, 2));
  if (result.code !== 0 && result.code !== 124) process.exitCode = 1;
}
main().catch(error => { console.error(error); process.exitCode = 1; });
