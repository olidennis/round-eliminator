#!/usr/bin/env python3
"""LLM: Exact diagnostic for the concrete degree-two RE fixture, with and without A B inputs."""

import argparse
import itertools
import json
from pathlib import Path
import shutil
import subprocess
import time

HERE = Path(__file__).resolve().parent


def vertices(bits):
    """LLM: Iterate original behavior IDs from a compact bitset."""
    while bits:
        bit = bits & -bits
        yield bit.bit_length() - 1
        bits ^= bit


def mask(ids):
    """LLM: Combine distinct original IDs into one bitset."""
    result = 0
    for vertex in ids:
        result |= 1 << vertex
    return result


def graphs(problem):
    """LLM: Build both exact behavior graphs from the original condensed passive relation."""
    assert problem['graph_class'] == {'active': [2], 'passive': [2]}
    assert problem['input']['kind'] == 'trivial'
    assert problem['mapping']['kind'] == 'unrestricted'
    count = len(problem['output']['labels'])
    partner = [None] * count
    for row in problem['output']['active']['degrees'][0]['configurations']:
        assert all(len(part['labels']) == 1 for part in row['parts'])
        a, b = [part['labels'][0] for part in row['parts'] for _ in range(part['multiplicity'])]
        assert partner[a] is None and (a == b or partner[b] is None)
        partner[a], partner[b] = b, a
    assert None not in partner

    # LLM: Each passive row contributes two rectangles, with endpoints interchangeable.
    # LLM: Permuting its right labels by the active partner involution is done once per row.
    normal, flipped = [0] * count, [0] * count
    for row in problem['output']['passive']['degrees'][0]['configurations']:
        a, b = [part['labels'] for part in row['parts'] for _ in range(part['multiplicity'])]
        am, bm = mask(a), mask(b)
        af, bf = mask(partner[x] for x in a), mask(partner[x] for x in b)
        for x in a:
            normal[x] |= bm
            flipped[x] |= bf
        for x in b:
            normal[x] |= am
            flipped[x] |= af
    joint = [flipped[a] & normal[partner[a]] for a in range(count)]
    assert all(not (joint[a] & (1 << a)) for a in range(count)), 'repeatable behavior: unbounded'
    representatives = mask(a for a in range(count) if a <= partner[a])
    plain = [joint[a] & joint[partner[a]] & representatives for a in range(count)]
    return partner, plain, joint, representatives


def greedy_clique(adj, active):
    """LLM: Obtain a checkable lower bound by repeatedly taking a compatible high-degree vertex."""
    clique = []
    while active:
        vertex = max(vertices(active), key=lambda v: (adj[v] & active).bit_count())
        clique.append(vertex)
        active &= adj[vertex]
    return clique


def color(adj, active, order):
    """LLM: Partition the graph into independent sets, giving a rigorous clique upper bound."""
    classes = []
    while active:
        candidates, group = active, 0
        for vertex in order:
            bit = 1 << vertex
            if candidates & bit:
                group |= bit
                active ^= bit
                candidates &= ~adj[vertex]
                candidates ^= bit
        classes.append(group)
    return classes


def best_coloring(adj, active, previous=()):
    """LLM: Compare cheap greedy orders and restrict any previous proper coloring."""
    ids = list(vertices(active))
    order = sorted(ids, key=lambda v: (adj[v] & active).bit_count())
    options = [color(adj, active, sequence) for sequence in (ids, ids[::-1], order, order[::-1])]
    if previous:
        options.append([group & active for group in previous if group & active])
    return min(options, key=len)


def dominate(adj, active):
    """LLM: Remove nonadjacent vertices whose neighborhoods are included in another's."""
    removed = []
    order = sorted(vertices(active), key=lambda v: (adj[v] & active).bit_count())
    for vertex in order:
        neighbors = adj[vertex] & active
        possible = active & ~adj[vertex] & ~(1 << vertex)
        for neighbor in itertools.islice(vertices(neighbors), 8):
            possible &= adj[neighbor]
            if not possible:
                break
        for replacement in vertices(possible):
            if neighbors & ~adj[replacement] == 0:
                removed.append([vertex, replacement])
                active ^= 1 << vertex
                break
    return active, removed


def complement_components(adj, active):
    """LLM: Split complement components; clique numbers of the original graph add across them."""
    components = []
    while active:
        frontier = active & -active
        active ^= frontier
        component = frontier
        while frontier:
            bit = frontier & -frontier
            frontier ^= bit
            discovered = active & ~adj[bit.bit_length() - 1]
            active ^= discovered
            frontier |= discovered
            component |= discovered
        components.append(component)
    return components


def build_solver(directory):
    """LLM: Build a small RustSAT/CaDiCaL graph solver outside production dependencies."""
    package = directory / 'solver'
    package.mkdir(exist_ok=True)
    (package / 'Cargo.toml').write_text(f'''[package]
name = "re3-path-clique-diagnostic"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
anyhow = "1"
serde_json = "1"
rustsat = {{ version = "=0.7.5", default-features = false }}
rustsat-cadical = {{ version = "=0.7.5", features = ["v2-2-1"] }}
[[bin]]
name = "path-clique"
path = {json.dumps(str(HERE / 'clique.rs'))}
''')
    if not (package / 'Cargo.lock').exists():
        shutil.copy(HERE.parents[4] / 'Cargo.lock', package / 'Cargo.lock')
    subprocess.run(['cargo', '+1.89.0', 'build', '--release', '--manifest-path', str(package / 'Cargo.toml')], check=True)
    return package / 'target/release/path-clique'


def solve(solver, directory, name, adj, active, initial, upper, extra=(), classes=()):
    """LLM: Encode nonedges and cardinality, retaining exact results and witness label IDs."""
    # LLM: Matching lower and coloring upper bounds already certify the answer.
    if upper == len(initial):
        return {'status': 'complete', 'maximum': upper, 'clique': initial, 'solver_seconds': 0}
    ids = list(vertices(active))
    index = {vertex: position for position, vertex in enumerate(ids)}
    nonedges = [[i, j] for i, a in enumerate(ids) for j, b in enumerate(ids[:i]) if not (adj[a] & (1 << b))]
    instance = dict(ids=ids, nonedges=nonedges, extra=list(extra), initial_clique=initial,
                    lower=len(initial) - len(extra), upper=upper - len(extra))
    if classes:
        instance['classes'] = [[index[v] for v in vertices(group & active)] for group in classes if group & active]
    path, result = directory / f'{name}-instance.json', directory / f'{name}-result.json'
    path.write_text(json.dumps(instance, separators=(',', ':')))
    subprocess.run([str(solver), str(path), str(result)], check=True)
    result = json.loads(result.read_text())
    assert result['status'] == 'complete', result
    return result


def verify(problem, partner, clique, paired):
    """LLM: Independently check a strategy directly against supplied passive rows, without graph bitsets."""
    rows = [[set(part['labels']) for part in row['parts'] for _ in range(part['multiplicity'])]
            for row in problem['output']['passive']['degrees'][0]['configurations']]

    def allowed(a, b):
        """LLM: Test an unordered concrete output pair in the original condensed union."""
        return any((a in first and b in second) or (a in second and b in first) for first, second in rows)

    assert len(clique) == len(set(clique))
    for i, a in enumerate(clique):
        for b in clique[:i]:
            if paired:
                assert allowed(a, partner[b]) and allowed(b, partner[a])
            else:
                assert all(allowed(x, y) for x in (a, partner[a]) for y in (b, partner[b]))


def main():
    """LLM: Certify both maxima using support graphs, safe reductions, and small exact graph SAT."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('--output', type=Path, default=Path('/tmp/re3-path-maxima'))
    parser.add_argument('--solver', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    solver = args.solver or build_solver(args.output)
    started = time.perf_counter()
    problem = json.loads(args.snapshot.read_text())
    partner, plain, joint, representatives = graphs(problem)
    proof = {}

    # LLM: Domination preserves the exact clique number. Universal vertices contribute
    # LLM: one each, leaving a much smaller graph for the first numerical maximum.
    active, removed = dominate(plain, representatives)
    classes = best_coloring(plain, active)
    components = complement_components(plain, active)
    extra = [next(vertices(component)) for component in components if component.bit_count() == 1]
    remaining = active & ~mask(extra)
    result_plain = solve(solver, args.output, 'plain', plain, remaining,
                         greedy_clique(plain, representatives), len(classes), extra)
    proof['plain_domination'] = removed
    verify(problem, partner, result_plain['clique'], False)

    # LLM: A plain strategy supplies two oriented colors for every behavior whose
    # LLM: two orientations are compatible. Other behaviors contribute one color.
    initial = []
    for vertex in result_plain['clique']:
        initial.append(vertex)
        if partner[vertex] != vertex and joint[vertex] & (1 << partner[vertex]):
            initial.append(partner[vertex])
    verify(problem, partner, initial, True)
    lower = len(initial)
    all_vertices = (1 << len(joint)) - 1
    previous = best_coloring(joint, all_vertices)
    active, removed = dominate(joint, all_vertices)
    proof['input_ab_domination'] = removed
    proof['input_ab_filtering'] = []

    # LLM: Search only for a clique larger than the already verified lower bound.
    # LLM: Degree and neighborhood-color bounds safely discard vertices unable to occur in it.
    while True:
        degree_drop = mask(v for v in vertices(active) if (joint[v] & active).bit_count() < lower)
        if degree_drop:
            proof['input_ab_filtering'].append({'degree_drop': list(vertices(degree_drop))})
            active &= ~degree_drop
            continue
        classes = best_coloring(joint, active, previous)
        dropped = mask(v for v in vertices(active) if 1 + sum(bool(joint[v] & group) for group in classes) <= lower)
        proof['input_ab_filtering'].append({'classes': [list(vertices(group)) for group in classes],
                                           'dropped': list(vertices(dropped))})
        previous = classes
        if not dropped:
            break
        active &= ~dropped
    active, removed = dominate(joint, active)
    proof['input_ab_final_domination'] = removed
    classes = [group & active for group in previous if group & active]
    result_joint = solve(solver, args.output, 'input_ab', joint, active, initial, max(lower, len(classes)), classes=classes)
    verify(problem, partner, result_joint['clique'], True)
    summary = {'plain': result_plain, 'input_ab': result_joint, 'seconds_excluding_build': time.perf_counter() - started}
    (args.output / 'proof.json').write_text(json.dumps(proof, separators=(',', ':')))
    (args.output / 'maxima.json').write_text(json.dumps(summary, indent=2))
    print(json.dumps(summary))


if __name__ == '__main__':
    main()
