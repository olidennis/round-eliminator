#!/usr/bin/env python3
"""Temporary, instrumented coloring kernels; production sources are untouched."""
import argparse
from pathlib import Path
import shutil

HERE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('destination', type=Path)
parser.add_argument('--variant', choices=['baseline', 'symmetry', 'batch', 'symmetry-batch', 'batsat', 'eager', 'symmetry-eager', 'probes', 'symmetry-eager-probes', 'grouped', 'symmetry-eager-grouped'], default='baseline')
parser.add_argument('--wasm', action='store_true', help='also copy the unchanged production wasm adapter')
args = parser.parse_args()
core = args.destination / 'core'
core.mkdir(parents=True, exist_ok=True)
shutil.copytree(HERE.parent.parent / 'src', core / 'src', dirs_exist_ok=True)
shutil.copytree(HERE.parent / 'support', core / 'benches/support', dirs_exist_ok=True)
(core / 'Cargo.toml').write_text((HERE.parent.parent / 'Cargo.toml').read_text().split('[[bench]]')[0])
# These are the pre-integration kernels measured in the investigation.
for name, relative in [('encoding.rs', 'solvability/coloring/encoding.rs'),
                       ('distance_two.rs', 'solvability/coloring/distance_two.rs'),
                       ('passive.rs', 'solvability/passive.rs'), ('sat.rs', 'sat.rs')]:
    shutil.copyfile(HERE / 'baseline' / name, core / 'src/algorithms' / relative)
# The surrounding API now accepts cached artifacts. Historical probes always
# use fresh snapshots; these adapters retain the original uncached kernels.
for filename, local_color in [('encoding.rs', True), ('distance_two.rs', False)]:
    path = core / 'src/algorithms/solvability/coloring' / filename
    source = path.read_text()
    marker = '    fn witness('
    adapter = """    pub(super) fn new_with_maximum(
        problem: &'a crate::model::Problem,
        KIND
        solver: S,
        _maximum: Option<&'a crate::algorithms::maximize::MaximizationResult>,
        computation: &mut crate::computation::Computation<'_>,
    ) -> anyhow::Result<Self> { Self::new(problem, ARGsolver, computation) }

""".replace('KIND', 'kind: super::ColoringKind,' if local_color else '').replace('ARG', 'kind, ' if local_color else '')
    path.write_text(source.replace(marker, adapter + marker, 1))
path = core / 'src/algorithms/solvability/passive.rs'
source = path.read_text()
source += """
pub(crate) fn forbidden_with_maximum<T: Copy + Ord, I: Copy + Ord>(
    constraint: &crate::model::Constraint, inputs: &[crate::model::CondensedConfiguration<I>],
    emissions: &[Emission<T,I>], mode: PassiveMode,
    _maximum: Option<&crate::algorithms::maximize::MaximizationResult>,
    _original_input: impl Fn(I) -> Option<crate::model::LabelId>,
    cancellation: &crate::computation::Cancellation,
) -> Result<Option<Vec<T>>, crate::computation::Cancelled> {
    forbidden_combination(constraint, inputs, emissions, mode, cancellation)
}
"""
adapter_start = source.index('pub(crate) fn forbidden_with_maximum')
adapter = source[adapter_start:]
source = source[:adapter_start]
pos = source.index('fn forbidden_output')
source = source[:pos] + adapter + '\n' + source[pos:]
path.write_text(source)
# The post-integration degree-ten regression deliberately exercises a query
# that the historical baseline times out on. Its experimental version remains
# in this script's diagnostic tests; avoid duplicating the new production test.
path = core / 'src/algorithms/solvability/coloring/tests.rs'
source = path.read_text()
path.write_text(source.split('#[test]\nfn re_matching_preserves_the_incidence_palette_boundary()')[0])


path = core / 'src/algorithms/solvability/coloring/encoding.rs'
source = path.read_text()
source = source.replace('iterations: u32,', 'iterations: u32,\n    diagnostic: Diagnostic,')
source = source.replace('iterations: 0,', 'iterations: 0,\n            diagnostic: Diagnostic::default(),')
source = source.replace('let literal = self.formula.new_lit();',
                        'let literal = self.formula.new_lit();\n                        self.diagnostic.memberships += 1;')
source = source.replace('let witness = self.witness(colors)?;',
                        'self.diagnostic.candidates += 1;\n            let witness = self.witness(colors)?;')
source = source.replace('let positions: Vec<_> = inputs',
                        'self.diagnostic.active_visits += 1;\n                        let positions: Vec<_> = inputs', 1)
source = source.replace('if let Some((context, palette)) = failed {',
                        'if let Some((context, palette)) = failed {\n                self.diagnostic.active_encodings += 1;')
source = source.replace('forbidden = forbidden_combination(',
                        'self.diagnostic.passive_visits += 1;\n                    forbidden = forbidden_combination(')
source = source.replace('if let Some(sources) = forbidden {',
                        '''if let Some(sources) = forbidden {
                *self.diagnostic.blocking_lengths.entry(sources.len()).or_default() += 1;''')
source += '''
#[derive(Default)]
struct Diagnostic {
    memberships: usize, candidates: usize, active_visits: usize,
    active_encodings: usize, passive_visits: usize,
    blocking_lengths: std::collections::BTreeMap<usize, usize>,
}
impl Drop for Diagnostic {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("{}", serde_json::json!({"event":"diagnostic", "memberships":self.memberships,
            "candidates":self.candidates,"active_visits":self.active_visits,
            "active_encodings":self.active_encodings,"passive_visits":self.passive_visits,
            "blocking_lengths":self.blocking_lengths}));
    }
}
'''
source += '''
#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use crate::computation::{Cancellation, Computation};
    use super::super::{PaletteSearch, ColoringResult, check_with, tests::plain, verify_witness};

    #[test]
    fn re_matching_preserves_the_incidence_palette_boundary() {
        for degree in [2, 3, 10] {
            let p = plain(&format!("M U^{}\\nP^{degree}", degree - 1),
                &format!("M UP^{}\\nU^{degree}", degree - 1));
            let p = crate::algorithms::round_elimination::apply(&p).unwrap();
            let mut report = |_| {};
            let mut queries = Vec::new();
            let result = check_with(&p, ColoringKind::IncidenceEdges, PaletteSearch::Maximum,
                PassiveMode::General, &mut Computation::new(Cancellation::default(), &mut report),
                &mut |bounds| { if let Some(colors) = bounds.testing { queries.push(colors); } }).unwrap();
            assert_eq!(queries, [degree, degree + 1]);
            let ColoringResult::Maximum { colors, witness } = result else { panic!("finite maximum expected") };
            assert_eq!(colors, degree);
            assert!(verify_witness(&p, ColoringKind::IncidenceEdges, &witness, false));
        }
    }

    #[test]
    fn incremental_solver_can_disable_and_reenable_colors() {
        let p = plain("A^2\\nB^2\\nC^2", "A BC\\nB C");
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        let mut solver = PaletteSolver::new(&p, ColoringKind::Vertex,
            crate::algorithms::sat::backend(&computation.cancellation), &mut computation).unwrap();
        for colors in [2, 3, 4, 2, 3, 5, 2] {
            let witness = solver.solve(colors, PassiveMode::General, &mut computation).unwrap();
            assert_eq!(witness.is_some(), colors <= 3);
            if let Some(witness) = witness {
                assert!(verify_witness(&p, ColoringKind::Vertex, &witness, false));
            }
        }
    }
}
'''
if args.variant in ['symmetry', 'symmetry-batch', 'symmetry-eager', 'symmetry-eager-probes', 'symmetry-eager-grouped']:
    # Color names are exchangeable for these three local-color kinds. Sort the
    # full membership bit vectors lexicographically; keep equal rows allowed.
    # The successor color gate disables the constraint for unused palettes.
    source = source.replace('self.memberships.push(behavior);', '''
            if let Some(previous) = self.memberships.last() {
                let left: Vec<_> = previous.iter().flatten().map(|choice| choice.literal).collect();
                let right: Vec<_> = behavior.iter().flatten().map(|choice| choice.literal).collect();
                let mut prefix_equal = enabled;
                for (&a, &b) in left.iter().zip(&right) {
                    self.formula.add_clause(&[!prefix_equal, !a, b])?;
                    let next = self.formula.new_lit();
                    self.formula.add_clause(&[!next, prefix_equal])?;
                    self.formula.add_clause(&[!next, !a, b])?;
                    self.formula.add_clause(&[!next, a, !b])?;
                    self.formula.add_clause(&[!prefix_equal, a, b, next])?;
                    self.formula.add_clause(&[!prefix_equal, !a, !b, next])?;
                    prefix_equal = next;
                }
            }
            self.memberships.push(behavior);''')
if args.variant in ['batch', 'symmetry-batch']:
    # Learn each color-placement's first counterexample instead of just the
    # first placement's counterexample. Every clause has existing provenance.
    source = source.replace('let mut forbidden = None;', 'let mut forbidden = None;\n            let mut learned = Vec::new();')
    source = source.replace('Ok(forbidden.is_none())', '''
                    if let Some(sources) = forbidden.take() { learned.push(sources); }
                    Ok(true)''')
    source = source.replace('if let Some(sources) = forbidden {', 'if !learned.is_empty() {\n              for sources in learned {')
    source = source.replace('''                continue;
            }
            return Ok(Some(witness));''', '''                }
                continue;
            }
            return Ok(Some(witness));''')
path.write_text(source)

# Instrument the separate neighbor-color solver with the same counters.
path = core / 'src/algorithms/solvability/coloring/distance_two.rs'
distance = path.read_text()
distance = distance.replace('iterations: u32,', 'iterations: u32,\n    diagnostic: Diagnostic,')
distance = distance.replace('iterations: 0,', 'iterations: 0,\n            diagnostic: Diagnostic::default(),')
distance = distance.replace('let literal = self.formula.new_lit();',
    'let literal = self.formula.new_lit();\n                                self.diagnostic.memberships += 1;')
distance = distance.replace('let witness = self.witness(colors)?;',
    'self.diagnostic.candidates += 1;\n            let witness = self.witness(colors)?;')
distance = distance.replace('let positions: Vec<_> = inputs',
    'self.diagnostic.active_visits += 1;\n                            let positions: Vec<_> = inputs', 1)
distance = distance.replace('if let Some((context, own, neighbors)) = failed {',
    'if let Some((context, own, neighbors)) = failed {\n                self.diagnostic.active_encodings += 1;')
distance = distance.replace('forbidden = forbidden_combination(',
    'self.diagnostic.passive_visits += 1;\n                    forbidden = forbidden_combination(')
distance = distance.replace('if let Some(sources) = forbidden {',
    'if let Some(sources) = forbidden {\n                *self.diagnostic.blocking_lengths.entry(sources.len()).or_default() += 1;')
distance += source[source.index('#[derive(Default)]\nstruct Diagnostic'):source.index('#[cfg(test)]\nmod diagnostic_tests')]
distance += '''
#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use crate::computation::{Cancellation, Computation};
    use super::super::{tests::plain, verify_witness};

    #[test]
    fn incremental_neighbor_solver_can_disable_and_reenable_colors() {
        let p = plain("O I", "O I");
        let mut report = |_| {};
        let mut computation = Computation::new(Cancellation::default(), &mut report);
        let mut solver = PaletteSolver::new(&p,
            crate::algorithms::sat::backend(&computation.cancellation), &mut computation).unwrap();
        for colors in [3, 4, 3, 5, 3] {
            let witness = solver.solve(colors, PassiveMode::General, &mut computation).unwrap();
            assert_eq!(witness.is_some(), colors == 3);
            if let Some(witness) = witness {
                assert!(verify_witness(&p, ColoringKind::DistanceTwo, &witness, false));
            }
        }
    }
}
'''
path.write_text(distance)

if args.variant in ['eager', 'symmetry-eager', 'symmetry-eager-probes', 'symmetry-eager-grouped']:
    path = core / 'src/algorithms/solvability/coloring/encoding.rs'
    source = path.read_text().replace('self.extend(colors)?;', '''
        let previous_colors = self.memberships.len() as u32;
        self.extend(colors)?;
        if colors > previous_colors {
            let mut emissions = Vec::new();
            for color in 0..colors as usize {
                for (input, choices) in self.inputs.iter().zip(&self.memberships[color]) {
                    emissions.extend(choices.iter().map(|choice| Emission {
                        input: (*input, color as u32), output: choice.label, source: choice.literal,
                    }));
                }
            }
            let low: Vec<_> = self.passive_inputs.iter().filter(|row| row.degree() <= 2).cloned().collect();
            prepare::passive_families(&low, self.kind, colors, &computation.cancellation, &mut |family| {
                if family.degree() != 0 && !family.parts().iter().any(|part|
                    part.labels().iter().any(|(_, color)| color >= previous_colors)) { return Ok(true); }
                crate::algorithms::solvability::passive::visit_forbidden(
                    self.problem.output().passive(), family, &emissions, &computation.cancellation, |sources| {
                        self.formula.add_clause(&sources.into_iter().map(|literal| !literal).collect::<Vec<_>>())
                    })?;
                Ok(true)
            })?;
        }
        let passive_inputs: Vec<_> = self.passive_inputs.iter().filter(|row| row.degree() > 2).cloned().collect();''')
    source = source.replace('&self.passive_inputs,\n                self.kind,', '&passive_inputs,\n                self.kind,')
    path.write_text(source)
    path = core / 'src/algorithms/solvability/coloring/distance_two.rs'
    source = path.read_text().replace('self.extend(colors)?;', '''
        let previous_colors = self.memberships.len() as u32;
        self.extend(colors)?;
        if colors > previous_colors {
            let mut emissions = Vec::new();
            for own in 0..colors as usize {
                for neighbor in 0..colors as usize {
                    for (input, choices) in self.inputs.iter().zip(&self.memberships[own][neighbor]) {
                        emissions.extend(choices.iter().map(|choice| Emission {
                            input: (*input, own as u32, neighbor as u32),
                            output: choice.label, source: choice.literal,
                        }));
                    }
                }
            }
            passive_families(&self.passive_inputs, colors, &computation.cancellation, &mut |family| {
                if !family.parts().iter().any(|part| part.labels().iter().any(|(_, own, neighbor)|
                    own >= previous_colors || neighbor >= previous_colors)) { return Ok(true); }
                crate::algorithms::solvability::passive::visit_forbidden(
                    self.problem.output().passive(), family, &emissions, &computation.cancellation, |sources| {
                        self.formula.add_clause(&sources.into_iter().map(|literal| !literal).collect::<Vec<_>>())
                    })?;
                Ok(true)
            })?;
        }''')
    start = source.index('            let mut emissions = Vec::new();', source.index('                continue;\n            }'))
    end = source.index('            return Ok(Some(witness));', start)
    source = source[:start] + source[end:]
    source = source.replace('        mode: PassiveMode,', '        _mode: PassiveMode,')
    path.write_text(source)

if args.variant in ['grouped', 'symmetry-eager-grouped']:
    path = core / 'src/algorithms/solvability/passive.rs'
    source = path.read_text()
    start = source.index('        let family = CondensedConfiguration::new(')
    end = source.index('.expect("transformation preserves degree");', start) + len('.expect("transformation preserves degree");')
    source = source[:start] + '        let family = output_family(&parts);' + source[end:]
    start = source.index('fn forbidden_output')
    source = source[:start] + '''
fn forbidden_output<T: Copy + Ord, I: Copy + Ord>(
    constraint: &Constraint,
    parts: &mut [OutputPart<'_, T, I>],
    cancellation: &Cancellation,
) -> Result<Option<Vec<T>>, Cancelled> {
    let mut grouped = BTreeMap::<Vec<LabelId>, Vec<&OutputPart<'_, T, I>>>::new();
    for part in parts.iter() {
        cancellation.check()?;
        grouped.entry(part.choices.iter().map(|emission| emission.output).collect())
            .or_default().push(part);
    }
    // Group only the semantic choices. Provenance stays attached to every
    // original part; equal output domains may have distinct input/color origins.
    let groups: Vec<_> = grouped.into_values().collect();
    let family = output_family(parts);
    let mut forbidden = None;
    visit_multisets_with(&family, cancellation, |output, counts| {
        if accepts_with(constraint, output, cancellation)? { return Ok(true); }
        let mut sources = Vec::new();
        for (origins, counts) in groups.iter().zip(counts) {
            let mut remaining = counts.clone();
            for origin in origins {
                cancellation.check()?;
                let mut slots = origin.multiplicity;
                for (emission, count) in origin.choices.iter().zip(&mut remaining) {
                    let used = slots.min(*count);
                    if used != 0 { sources.push(emission.source); }
                    slots -= used;
                    *count -= used;
                }
                debug_assert_eq!(slots, 0);
            }
            debug_assert!(remaining.iter().all(|count| *count == 0));
        }
        sources.sort_unstable();
        sources.dedup();
        forbidden = Some(sources);
        Ok(false)
    })?;
    Ok(forbidden)
}

fn output_family<T, I>(parts: &[OutputPart<'_, T, I>]) -> CondensedConfiguration {
    let normalized = Configuration::new(parts.iter().map(|part| (
        LabelSet::new(part.choices.iter().map(|emission| emission.output).collect()),
        part.multiplicity,
    )).collect()).expect("same total degree");
    CondensedConfiguration::new(normalized.multiplicities().iter().map(|(labels, count)|
        Part::new(labels.clone(), *count).expect("nonempty choices")
    ).collect()).expect("same degree")
}

#[cfg(test)]
mod grouped_diagnostic_tests {
    use super::*;

    // Independent oracle: allowed iff the number of A outputs is not three.
    fn safe(mask: u8) -> bool {
        let counts = [1, 2, 4];
        let mut totals = vec![0];
        for (input, count) in counts.into_iter().enumerate() {
            let a = mask & (1 << (2 * input)) != 0;
            let b = mask & (1 << (2 * input + 1)) != 0;
            if !a && !b { return true; } // this emission family cannot occur
            totals = totals.into_iter().flat_map(|total| {
                (0..=count).filter(move |n| (*n == 0 || a) && (*n == count || b))
                    .map(move |n| total + n)
            }).collect();
        }
        !totals.contains(&3)
    }

    #[test]
    fn every_grouped_counterexample_clause_preserves_input_origins() {
        let target = Constraint::new((0..=7).filter(|count| *count != 3).map(|count|
            CondensedConfiguration::new(vec![
                Part::new(LabelSet::new(vec![LabelId(0)]), count).unwrap(),
                Part::new(LabelSet::new(vec![LabelId(1)]), 7 - count).unwrap(),
            ]).unwrap()
        ).collect());
        let input = CondensedConfiguration::new([1, 2, 4].into_iter().enumerate().map(|(input, count)|
            Part::new(LabelSet::new(vec![input as u8]), count).unwrap()
        ).collect()).unwrap();
        for mask in 0u8..64 {
            let emissions: Vec<_> = (0u8..6).filter(|source| mask & (1 << source) != 0)
                .map(|source| Emission { input: source / 2,
                    output: LabelId(u32::from(source % 2)), source }).collect();
            let forbidden = forbidden_combination(&target, std::slice::from_ref(&input),
                &emissions, PassiveMode::General, &Cancellation::default()).unwrap();
            assert_eq!(forbidden.is_none(), safe(mask), "mask {mask}");
            if let Some(sources) = forbidden {
                assert!(!sources.is_empty());
                let required = sources.into_iter().fold(0, |bits, source| bits | (1 << source));
                assert_eq!(mask & required, required);
                for future in 0u8..64 {
                    if future & required == required {
                        assert!(!safe(future), "clause admitted a valid future assignment");
                    }
                }
            }
        }
    }
}
'''
    path.write_text(source)

if args.variant in ['probes', 'symmetry-eager-probes']:
    path = core / 'src/algorithms/solvability/passive.rs'
    source = path.read_text()
    start = source.index('    let family = CondensedConfiguration::new(', source.index('fn forbidden_output'))
    source = source[:start] + '''
    // Same nearly homogeneous witness heuristic already used by covers_with;
    // retain emission origins so each rejection is a sound blocking clause.
    let preferred: std::collections::BTreeSet<_> = parts.iter()
        .flat_map(|part| part.choices.iter().map(|choice| choice.output)).collect();
    for label in preferred {
        cancellation.check()?;
        let mut selected = Vec::new();
        let mut occurrences = Vec::new();
        for part in parts.iter() {
            cancellation.check()?;
            let choice = part.choices.iter().copied().find(|choice| choice.output == label)
                .unwrap_or(part.choices[0]);
            selected.push(choice.source);
            occurrences.push((choice.output, part.multiplicity));
        }
        let concrete = Configuration::new(occurrences).expect("same degree");
        if !accepts_with(constraint, &concrete, cancellation)? {
            selected.sort_unstable();
            selected.dedup();
            return Ok(Some(selected));
        }
    }
''' + source[start:]
    path.write_text(source)

if args.variant == 'batsat':
    path = core / 'src/algorithms/sat.rs'
    source = path.read_text().replace('#[cfg(not(target_arch = "wasm32"))]', '#[cfg(any())]').replace(
        '#[cfg(target_arch = "wasm32")]', '#[cfg(all())]')
    path.write_text(source)
    path = core / 'Cargo.toml'
    path.write_text(path.read_text().replace(
        '[target.\'cfg(not(target_arch = "wasm32"))\'.dependencies]',
        '[target.\'cfg(not(target_arch = "wasm32"))\'.dependencies]\nrustsat-batsat = "0.7.5"'))
# Count all submitted CNF clauses, including library cardinality encoders.
path = core / 'src/algorithms/sat.rs'
source = path.read_text().replace('variables: BasicVarManager,', 'variables: BasicVarManager,\n    diagnostic_clauses: usize,')
source = source.replace('variables: BasicVarManager::default(),', 'variables: BasicVarManager::default(),\n            diagnostic_clauses: 0,')
source = source.replace('self.solver.add_clause_ref(literals)', 'self.diagnostic_clauses += 1;\n        self.solver.add_clause_ref(literals)')
source = source.replace('match clauses.error {', 'self.diagnostic_clauses += clauses.count;\n        match clauses.error {')
source += '''
impl<S> Drop for Formula<S> {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("{}", serde_json::json!({"event":"formula", "variables":self.variables.n_used(),
            "clauses":self.diagnostic_clauses}));
    }
}
'''
path.write_text(source)
if args.wasm:
    adapter = args.destination / 'wasm'
    adapter.mkdir(parents=True, exist_ok=True)
    original = HERE.parents[2] / 'wasm'
    shutil.copytree(original / 'src', adapter / 'src', dirs_exist_ok=True)
    (adapter / 'Cargo.toml').write_text((original / 'Cargo.toml').read_text() + '\n[workspace]\n')
print(core)
