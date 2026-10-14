#!/usr/bin/env python3
"""Sequential kernel measurements; build first, then run without other jobs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--variant', required=True)
parser.add_argument('--steps', type=int, default=1)
parser.add_argument('--palette', default='maximum')
parser.add_argument('--deadline', type=int, default=15)
parser.add_argument('--samples', type=int, default=3)
parser.add_argument('--sample-start', type=int, default=1)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--kind', default='incidence_edges')
parser.add_argument('--definition', type=Path)
parser.add_argument('--case')
parser.add_argument('--passive-mode', default='general', choices=['general', 'maximized', 'premaximized'])
args = parser.parse_args()
case = None
if args.case:
    case = json.loads((Path(__file__).parent / 'cases.json').read_text())[args.case]
    args.kind = case['kind']
    args.steps = case.get('steps', 0)
    args.palette = str(case['palette'])
binary_hash = hashlib.sha256(args.binary.read_bytes()).hexdigest()
with args.output.open('a') as output:
    for sample in range(args.sample_start, args.sample_start + args.samples):
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        child = subprocess.run([str(args.binary), '10', str(args.steps), args.palette, str(args.deadline), args.passive_mode],
            text=True, capture_output=True, timeout=args.deadline + 30, check=True,
            env={**os.environ, 'RE3_COLORING_KIND': args.kind,
                 **({'RE3_DEFINITION_JSON': json.dumps(case['definition'])} if case else {}),
                 **({'RE3_DEFINITION': str(args.definition.resolve())} if args.definition else {})})
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        events = [json.loads(line) for line in child.stdout.splitlines()]
        diagnostic = [json.loads(line) for line in child.stderr.splitlines() if line.startswith('{')]
        row = {'variant': args.variant, 'steps': args.steps, 'palette': args.palette, 'sample': sample,
            'kind': args.kind, 'case': args.case,
            'passive_mode': args.passive_mode,
            'definition': case['definition'] if case else str(args.definition) if args.definition else 'degree-ten-bmm',
            'binary_sha256': binary_hash,
            'total_cpu_seconds': after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
            'events': events, 'diagnostic': diagnostic}
        output.write(json.dumps(row) + '\n')
        output.flush()
        print(json.dumps({**{key: row[key] for key in ['variant', 'sample', 'total_cpu_seconds']},
            'result': events[-1]}), flush=True)
