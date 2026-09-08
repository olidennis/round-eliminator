// Sequential native, fixed-CNF benchmarks. No known solution is passed to solvers.
// Usage: node benchmark.cjs ARTIFACT_DIRECTORY EXPORTER [SECONDS=180] [SOLVER...]
// ARTIFACT_DIRECTORY contains proof-21.cnf, known.model and cadical/build/cadical,
// kissat/build/kissat. Each named result file must not already exist.
// Also supports gimsatul/gimsatul; options: --threads=N (default 4 for
// Gimsatul), --depth=N, --symmetry. Restricted instances/models are named
// proof-21-depth8[-symmetry].cnf and known-depth8[-symmetry].model, etc.
const fs = require('node:fs');
const path = require('node:path');
const readline = require('node:readline');
const { spawn } = require('node:child_process');
const crypto = require('node:crypto');

async function checkModel(cnfPath, modelPath) {
  const text = fs.readFileSync(modelPath, 'utf8');
  if (!/^s SATISFIABLE$/m.test(text)) throw new Error('Missing SAT result');
  let values, declaredClauses, clauses = 0, satisfied = false;
  const literals = [];
  for (const line of text.split('\n')) {
    if (line.startsWith('v ')) {
      for (const word of line.slice(2).trim().split(/\s+/)) {
        const lit = Number(word);
        if (!Number.isInteger(lit)) throw new Error('Malformed model literal');
        if (lit) literals.push(lit);
      }
    }
  }
  for await (const line of readline.createInterface({ input: fs.createReadStream(cnfPath) })) {
    if (line.startsWith('c') || !line.trim()) continue;
    if (line.startsWith('p ')) {
      const [, kind, n, m] = line.split(/\s+/);
      if (kind !== 'cnf') throw new Error('Expected CNF');
      values = new Int8Array(Number(n) + 1);
      declaredClauses = Number(m);
      for (const lit of literals) {
        const variable = Math.abs(lit), sign = Math.sign(lit);
        if (variable >= values.length) throw new Error('Model variable out of range');
        if (values[variable] && values[variable] !== sign) throw new Error('Conflicting model');
        values[variable] = sign;
      }
      continue;
    }
    if (!values) throw new Error('Missing CNF header');
    for (const word of line.trim().split(/\s+/)) {
      const lit = Number(word);
      if (!Number.isInteger(lit) || Math.abs(lit) >= values.length) throw new Error('Invalid CNF');
      if (lit === 0) {
        if (!satisfied) throw new Error(`Model falsifies clause ${clauses + 1}`);
        clauses++;
        satisfied = false;
      } else if (values[Math.abs(lit)] === Math.sign(lit)) satisfied = true;
    }
  }
  if (clauses !== declaredClauses) throw new Error('Clause count mismatch');
  return { variables: values.length - 1, clauses };
}

function execute(command, args, stdout, stderr) {
  return new Promise((resolve, reject) => {
    const out = fs.openSync(stdout, 'wx'), err = fs.openSync(stderr, 'wx');
    const child = spawn(command, args, { env: { ...process.env, RE_NUM_THREADS: '1' },
      stdio: ['ignore', out, err] });
    fs.closeSync(out);
    fs.closeSync(err);
    child.on('error', reject);
    child.on('close', (code, signal) => resolve({ code, signal }));
  });
}

async function main() {
  const [directory, tool, secondsText = '180', ...extra] = process.argv.slice(2);
  if (!directory || !tool) throw new Error('Expected ARTIFACT_DIRECTORY EXPORTER [SECONDS] [SOLVER...]');
  const root = path.resolve(directory), exporter = path.resolve(tool), seconds = Number(secondsText);
  if (!Number.isInteger(seconds) || seconds < 1) throw new Error('Invalid time limit');
  const selected = extra.filter(arg => !arg.startsWith('--'));
  const restrictions = extra.filter(arg => arg.startsWith('--depth=') || arg === '--symmetry');
  const depth = restrictions.find(arg => arg.startsWith('--depth='))?.slice(8);
  const symmetric = restrictions.includes('--symmetry');
  const threadOption = extra.find(arg => arg.startsWith('--threads='));
  const threads = threadOption ? Number(threadOption.slice(10)) : 4;
  if (!Number.isInteger(threads) || threads < 1) throw new Error('Invalid thread count');
  if (depth !== undefined && (!/^\d+$/.test(depth) || Number(depth) < 1)) throw new Error('Invalid depth');
  if (extra.some(arg => arg.startsWith('--') && !restrictions.includes(arg) && arg !== threadOption)) {
    throw new Error('Unknown benchmark option');
  }
  const variant = (depth !== undefined ? `-depth${depth}` : '') + (symmetric ? '-symmetry' : '');
  const cnf = path.join(root, `proof-21${variant}.cnf`);
  const problem = path.join(__dirname, 'hard_nonexistence.txt');
  console.log('Verifying known model against the actual DIMACS file (not a regenerated formula).');
  const instance = await checkModel(cnf, path.join(root, `known${variant}.model`));
  const hash = crypto.createHash('sha256').update(fs.readFileSync(cnf)).digest('hex');
  console.log(JSON.stringify({ ...instance, sha256: hash }));
  const names = selected.length ? selected : ['cadical', 'kissat', 'minisat'];
  for (const name of names) {
    const parallel = name === 'gimsatul';
    const cpus = parallel && threads > 1 ? `0-${threads - 1}` : '0';
    const prefix = path.join(root, `${name}${parallel ? `-t${threads}` : ''}${variant}-${seconds}s`);
    const model = `${prefix}.model`;
    let command, args;
    if (name === 'minisat') {
      command = exporter;
      args = ['solve', cnf, model];
    } else if (parallel) {
      command = path.join(root, 'gimsatul', 'gimsatul');
      args = [`--threads=${threads}`, cnf];
    } else if (/^(cadical|kissat)(-sat)?$/.test(name)) {
      const solver = name.split('-')[0];
      command = path.join(root, solver, 'build', solver);
      args = [...(name.endsWith('-sat') ? ['--sat'] : []), cnf];
    } else throw new Error(`Unknown solver ${name}`);
    for (const suffix of ['.stdout', '.stderr', '.time', '.json', '.model', '.certificate', '.verify.stderr']) {
      if (fs.existsSync(prefix + suffix)) throw new Error(`Refusing to overwrite ${prefix + suffix}`);
    }
    const timing = '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"peak_rss_kib":%M,"exit":%x}';
    const invocation = ['-f', timing, '-o', `${prefix}.time`, 'taskset', '-c', cpus,
      'timeout', '--signal=INT', '--kill-after=5s', `${seconds}s`, command, ...args];
    console.log(`${new Date().toISOString()} START ${name}${variant} (${seconds}s wall limit, CPUs ${cpus})`);
    const exit = await execute('/usr/bin/time', invocation, `${prefix}.stdout`, `${prefix}.stderr`);
    const stdout = fs.readFileSync(`${prefix}.stdout`, 'utf8');
    const timingLines = fs.readFileSync(`${prefix}.time`, 'utf8').trim().split('\n');
    const resource = JSON.parse(timingLines.find(line => line.startsWith('{')));
    const status = /^s SATISFIABLE$/m.test(stdout) ? 'SAT' :
      /^s UNSATISFIABLE$/m.test(stdout) ? 'UNSAT' : exit.code === 124 ? 'TIMEOUT' : 'UNKNOWN';
    let verified = false;
    if (status === 'SAT') {
      const actualModel = name === 'minisat' ? model : `${prefix}.stdout`;
      await checkModel(cnf, actualModel);
      const validation = await execute(exporter, ['verify', problem, '21', actualModel, ...restrictions],
        `${prefix}.certificate`, `${prefix}.verify.stderr`);
      if (validation.code !== 0) throw new Error(`${name} model failed certificate validation`);
      verified = true;
    }
    const result = { solver: name, status, verified, limit_seconds: seconds, cpus,
      threads: parallel ? threads : 1, restrictions,
      command: [command, ...args], invocation: ['/usr/bin/time', ...invocation],
      ...exit, ...resource, cnf_sha256: hash, ...instance };
    fs.writeFileSync(`${prefix}.json`, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
    console.log(JSON.stringify(result));
    if (status === 'UNSAT') throw new Error('Contradiction: instance has a verified satisfying assignment');
    if (exit.code !== 0 && exit.code !== 10 && exit.code !== 124) throw new Error(`Unexpected solver exit ${exit.code}`);
  }
}

if (require.main === module) main().catch(error => { console.error(error); process.exitCode = 1; });
module.exports = { checkModel };
