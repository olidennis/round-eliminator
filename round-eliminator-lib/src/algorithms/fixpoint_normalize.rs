//! Construct a lattice-based relaxation from an occurrence core. SAT is used
//! only for homomorphisms to proper subproblems, never to synthesize a lattice.
//! The guarantee assumes a live, nontrivial, unoriented node-edge weak fixed
//! point. We also check the constructed result: failure is NOT a certificate
//! that no fixed point exists. See docs/fixpoint-normalization.md.

use std::collections::{BTreeSet, HashMap};

use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Lit, TernaryVal};
use rustsat_minisat::{core::Minisat, Limit};

use super::{event::EventHandler, fixpoint_sat::name_fixed_point_with_mapping};
use crate::{
    constraint::Constraint,
    group::{Group, GroupType, Label},
    line::{Degree, Line},
    part::Part,
    problem::Problem,
};

#[derive(Clone, Debug)]
pub struct NormalizationOptions {
    pub max_labels: usize,
    pub max_degree: usize,
    pub max_occurrences: usize,
    pub max_choices: usize,
    pub max_lattice_nodes: usize,
    pub max_sat_variables: usize,
    pub max_sat_clauses: usize,
    /// Per core-reduction query; exhausting it is inconclusive, not UNSAT.
    pub sat_conflicts: u32,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            max_labels: 64,
            max_degree: 16,
            max_occurrences: 192,
            max_choices: 100_000,
            max_lattice_nodes: 128,
            max_sat_variables: 500_000,
            max_sat_clauses: 2_000_000,
            sat_conflicts: 50_000,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NormalizationStats {
    pub original_configurations: usize,
    pub core_configurations: usize,
    pub core_occurrences: usize,
    pub core_queries: usize,
    pub lattice_nodes: usize,
}

#[derive(Debug)]
pub struct NormalizedFixedPoint {
    pub problem: Problem,
    /// Complete reflexive order of the construction lattice, verified equal
    /// to the maximal edge-replacement diagram of `problem`.
    pub diagram: Vec<(Label, Label)>,
    pub mapping: Vec<(Label, Label)>,
    /// Custom-diagram syntax, applied to the original input problem.
    pub diagram_text: String,
    pub stats: NormalizationStats,
}

fn check(eh: &EventHandler) -> Result<(), String> {
    if eh.is_cancelled() {
        Err("Lattice normalization cancelled (inconclusive)".into())
    } else {
        Ok(())
    }
}

fn limit(what: &str) -> String {
    format!("Lattice normalization inconclusive: {what} limit reached. No nonexistence conclusion was drawn.")
}

/// Enumerate multisets without first allocating an unbounded Cartesian product.
fn choices(
    c: &Constraint,
    degree: usize,
    options: &NormalizationOptions,
    eh: &mut EventHandler,
) -> Result<Vec<Vec<Label>>, String> {
    let mut result = BTreeSet::new();
    let mut work = 0usize;
    for line in &c.lines {
        let mut domains = Vec::new();
        for part in &line.parts {
            let GroupType::Many(count) = part.gtype else {
                return Err("Lattice normalization requires finite degrees".into());
            };
            for _ in 0..count {
                domains.push(part.group.as_vec());
            }
        }
        if domains.len() != degree {
            return Err("Inconsistent constraint degree".into());
        }
        if domains.iter().any(Vec::is_empty) {
            continue;
        }
        let count = domains
            .iter()
            .try_fold(1usize, |n, d| n.checked_mul(d.len()))
            .ok_or_else(|| limit("constraint expansion"))?;
        work = work
            .checked_add(count)
            .ok_or_else(|| limit("constraint expansion"))?;
        if work > options.max_choices {
            return Err(limit("constraint expansion"));
        }
        for index in 0..count {
            if index % 1024 == 0 {
                eh.notify("Normalize: expanding constraints", index, count);
                check(eh)?;
            }
            let mut remainder = index;
            let mut tuple = Vec::new();
            for domain in &domains {
                tuple.push(domain[remainder % domain.len()]);
                remainder /= domain.len();
            }
            tuple.sort_unstable();
            result.insert(tuple);
        }
    }
    Ok(result.into_iter().collect())
}

struct CoreSat<'a, 'b> {
    solver: Minisat,
    variables: usize,
    clauses: usize,
    options: &'a NormalizationOptions,
    eh: &'a mut EventHandler<'b>,
}

impl CoreSat<'_, '_> {
    fn var(&mut self) -> Result<Lit, String> {
        if self.variables >= self.options.max_sat_variables
            || self.variables >= (u32::MAX >> 1) as usize
        {
            return Err(limit("core SAT variable"));
        }
        let lit = Lit::new(self.variables as u32, false);
        self.variables += 1;
        Ok(lit)
    }

    fn clause(&mut self, clause: impl IntoIterator<Item = Lit>) -> Result<(), String> {
        if self.clauses >= self.options.max_sat_clauses {
            return Err(limit("core SAT clause"));
        }
        self.clauses += 1;
        if self.clauses % 4096 == 0 {
            self.eh
                .notify("Normalize: encoding core map", self.clauses, 0);
            check(self.eh)?;
        }
        self.solver
            .add_clause(clause.into_iter().collect())
            .map_err(|e| e.to_string())
    }

    /// Linear-size sequential at-most-one encoding.
    fn at_most_one(&mut self, values: &[Lit]) -> Result<(), String> {
        if values.len() < 2 {
            return Ok(());
        }
        let mut previous = self.var()?;
        self.clause([!values[0], previous])?;
        for &value in &values[1..values.len() - 1] {
            let next = self.var()?;
            self.clause([!value, next])?;
            self.clause([!previous, next])?;
            self.clause([!value, !previous])?;
            previous = next;
        }
        self.clause([!values[values.len() - 1], !previous])
    }

    fn one_hot(&mut self, values: &[Lit]) -> Result<(), String> {
        self.clause(values.iter().copied())?;
        self.at_most_one(values)
    }
}

/// All occurrence IDs are global: block * degree + port. Repeated original
/// labels remain DIFFERENT target occurrences. Returning just label types
/// here would incorrectly allow non-bijective maps inside a node tuple.
fn core_map(
    nodes: &[Vec<usize>],
    edge: &[u64],
    blocks: &[usize],
    omitted: usize,
    options: &NormalizationOptions,
    eh: &mut EventHandler,
) -> Result<Option<Vec<usize>>, String> {
    check(eh)?;
    let degree = nodes[0].len();
    let targets: Vec<_> = blocks.iter().copied().filter(|&b| b != omitted).collect();
    if targets.is_empty() {
        return Ok(None);
    }
    let types: Vec<_> = targets
        .iter()
        .flat_map(|&b| nodes[b].iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let positions: HashMap<_, _> = types.iter().enumerate().map(|(i, &a)| (a, i)).collect();
    let source: Vec<_> = blocks
        .iter()
        .flat_map(|&b| (0..degree).map(move |i| b * degree + i))
        .collect();
    let mut enc = CoreSat {
        solver: Minisat::default(),
        variables: 0,
        clauses: 0,
        options,
        eh,
    };
    let mut outputs = Vec::new();
    for _ in &source {
        let row: Vec<_> = (0..types.len())
            .map(|_| enc.var())
            .collect::<Result<_, _>>()?;
        enc.one_hot(&row)?;
        outputs.push(row);
    }
    let mut matches = Vec::new();
    for (r, _) in blocks.iter().enumerate() {
        let selected: Vec<_> = targets
            .iter()
            .map(|_| enc.var())
            .collect::<Result<_, _>>()?;
        enc.one_hot(&selected)?;
        for (t, &block) in targets.iter().enumerate() {
            let mut matrix = Vec::new();
            for i in 0..degree {
                let row: Vec<_> = (0..degree).map(|_| enc.var()).collect::<Result<_, _>>()?;
                enc.clause(std::iter::once(!selected[t]).chain(row.iter().copied()))?;
                enc.at_most_one(&row)?;
                for (j, &lit) in row.iter().enumerate() {
                    enc.clause([!lit, selected[t]])?;
                    enc.clause([!lit, outputs[r * degree + i][positions[&nodes[block][j]]]])?;
                    matches.push((r * degree + i, block * degree + j, lit));
                }
                matrix.push(row);
            }
            for j in 0..degree {
                let column: Vec<_> = matrix.iter().map(|row| row[j]).collect();
                enc.at_most_one(&column)?;
            }
        }
    }
    for (i, &o) in source.iter().enumerate() {
        for (j, &p) in source.iter().enumerate().skip(i) {
            if edge[nodes[o / degree][o % degree]] & (1 << nodes[p / degree][p % degree]) == 0 {
                continue;
            }
            // Include i == j: source self-pairs must remain self-compatible.
            for (a, &ta) in types.iter().enumerate() {
                for (b, &tb) in types.iter().enumerate() {
                    if edge[ta] & (1 << tb) == 0 {
                        enc.clause([!outputs[i][a], !outputs[j][b]])?;
                    }
                }
            }
        }
    }
    enc.eh
        .notify("Normalize: solving core map", blocks.len(), 0);
    check(enc.eh)?;
    enc.solver
        .set_limit(Limit::Conflicts(options.sat_conflicts as i64));
    let solved = enc.solver.solve().map_err(|e| e.to_string())?;
    check(enc.eh)?;
    match solved {
        SolverResult::Unsat => return Ok(None),
        SolverResult::Interrupted => return Err(limit("core SAT conflict")),
        SolverResult::Sat => {}
    }
    let solution = enc.solver.full_solution().map_err(|e| e.to_string())?;
    let mut image = vec![usize::MAX; nodes.len() * degree];
    for (i, target, lit) in matches {
        if solution.lit_value(lit) == TernaryVal::True {
            if image[source[i]] != usize::MAX {
                return Err("Invalid core witness: duplicate output".into());
            }
            image[source[i]] = target;
        }
    }
    validate_core_map(nodes, edge, blocks, &targets, &image)?;
    Ok(Some(image))
}

fn validate_core_map(
    nodes: &[Vec<usize>],
    edge: &[u64],
    blocks: &[usize],
    targets: &[usize],
    image: &[usize],
) -> Result<(), String> {
    let degree = nodes[0].len();
    let mut source = Vec::new();
    for &block in blocks {
        let mut outputs: Vec<_> = (0..degree).map(|i| image[block * degree + i]).collect();
        outputs.sort_unstable();
        let t = outputs[0] / degree;
        if !targets.contains(&t) || outputs != (t * degree..(t + 1) * degree).collect::<Vec<_>>() {
            return Err("Invalid core witness: target node is not a permutation".into());
        }
        source.extend(block * degree..(block + 1) * degree);
    }
    let label = |o: usize| nodes[o / degree][o % degree];
    for &o in &source {
        for &p in &source {
            if edge[label(o)] & (1 << label(p)) != 0
                && edge[label(image[o])] & (1 << label(image[p])) == 0
            {
                return Err("Invalid core witness: incompatible edge outputs".into());
            }
        }
    }
    Ok(())
}

fn common_neighbors(set: u64, universe: u64, edge: &[u64]) -> u64 {
    let mut result = universe;
    for (a, &neighbors) in edge.iter().enumerate() {
        if set & (1 << a) != 0 {
            result &= neighbors;
        }
    }
    result
}

fn closure(set: u64, universe: u64, edge: &[u64]) -> u64 {
    common_neighbors(common_neighbors(set, universe, edge), universe, edge)
}

fn lattice(
    universe: u64,
    edge: &[u64],
    options: &NormalizationOptions,
    eh: &mut EventHandler,
) -> Result<Vec<u64>, String> {
    let mut closed = BTreeSet::from([universe]);
    if options.max_lattice_nodes == 0 {
        return Err(limit("lattice size"));
    }
    for (a, &neighbors) in edge.iter().enumerate() {
        if universe & (1 << a) == 0 {
            continue;
        }
        check(eh)?;
        let previous: Vec<_> = closed.iter().copied().collect();
        for set in previous {
            closed.insert(set & neighbors);
            if closed.len() > options.max_lattice_nodes {
                return Err(limit("lattice size"));
            }
        }
        eh.notify(
            "Normalize: constructing neighborhood lattice",
            closed.len(),
            0,
        );
    }
    Ok(closed.into_iter().collect())
}

fn singleton_line(tuple: impl IntoIterator<Item = Label>) -> Line {
    let mut line = Line {
        parts: tuple
            .into_iter()
            .map(|a| Part {
                group: Group::from(vec![a]),
                gtype: GroupType::ONE,
            })
            .collect(),
    };
    line.normalize();
    line
}

impl Problem {
    /// Direct construction, not diagram synthesis. Input fixedness is an
    /// assumption of the completeness theorem, not an unchecked output claim:
    /// only a successful full procedure with checked nontriviality is returned.
    pub fn normalize_fixed_point(
        &self,
        options: &NormalizationOptions,
        eh: &mut EventHandler,
    ) -> Result<NormalizedFixedPoint, String> {
        check(eh)?;
        if self.orientation_given.is_some() || self.passive.degree != Degree::Finite(2) {
            return Err(
                "Lattice normalization supports unoriented node-edge problems only (edge degree 2)"
                    .into(),
            );
        }
        let Degree::Finite(degree) = self.active.degree else {
            return Err("Lattice normalization requires a positive finite node degree".into());
        };
        if degree == 0 {
            return Err("Lattice normalization requires a positive node degree".into());
        }
        if degree > options.max_degree {
            return Err(limit("node degree"));
        }
        let mut labels = self.labels();
        labels.sort_unstable();
        if labels.len() > options.max_labels.min(64) {
            return Err(limit("alphabet size (at most 64)"));
        }
        let label_index: HashMap<_, _> = labels.iter().enumerate().map(|(i, &a)| (a, i)).collect();
        let tuples = choices(&self.active, degree, options, eh)?;
        if tuples.is_empty() {
            return Err("Lattice normalization requires a nonempty live node constraint".into());
        }
        if tuples.len().saturating_mul(degree) > options.max_occurrences {
            return Err(limit("occurrence count"));
        }
        let nodes: Vec<Vec<_>> = tuples
            .iter()
            .map(|t| t.iter().map(|a| label_index[a]).collect())
            .collect();
        let pairs = choices(&self.passive, 2, options, eh)?;
        let mut edge = vec![0u64; labels.len()];
        for pair in &pairs {
            let (a, b) = (label_index[&pair[0]], label_index[&pair[1]]);
            edge[a] |= 1 << b;
            edge[b] |= 1 << a;
        }
        let used = nodes.iter().flatten().fold(0u64, |set, &a| set | (1 << a));
        for a in 0..labels.len() {
            if used & (1 << a) == 0 || edge[a] & used == 0 {
                return Err("Lattice normalization requires live labels: remove labels unused by nodes or without an edge partner first".into());
            }
        }
        if nodes.iter().any(|node| {
            node.iter()
                .all(|&a| node.iter().all(|&b| edge[a] & (1 << b) != 0))
        }) {
            return Err(
                "The input is zero-round solvable; a nontrivial relaxation cannot be produced"
                    .into(),
            );
        }
        let mut stats = NormalizationStats {
            original_configurations: nodes.len(),
            ..Default::default()
        };
        let mut blocks: Vec<_> = (0..nodes.len()).collect();
        let mut image: Vec<_> = (0..nodes.len() * degree).collect();
        loop {
            let mut reduced = false;
            // Stable ordering, no random restarts. Every successful query
            // strictly decreases the number of occurrence-labelled node blocks.
            for &omitted in &blocks {
                eh.notify(
                    "Normalize: reducing occurrence core",
                    blocks.len(),
                    nodes.len(),
                );
                stats.core_queries += 1;
                if let Some(map) = core_map(&nodes, &edge, &blocks, omitted, options, eh)? {
                    let targets: BTreeSet<_> = blocks
                        .iter()
                        .flat_map(|&b| (0..degree).map(move |i| b * degree + i))
                        .map(|o| map[o] / degree)
                        .collect();
                    for value in &mut image {
                        *value = map[*value];
                    }
                    blocks = targets.into_iter().collect();
                    reduced = true;
                    break;
                }
            }
            if !reduced {
                break;
            }
        }
        validate_core_map(
            &nodes,
            &edge,
            &(0..nodes.len()).collect::<Vec<_>>(),
            &blocks,
            &image,
        )?;
        stats.core_configurations = blocks.len();
        stats.core_occurrences = blocks.len() * degree;
        let universe = blocks
            .iter()
            .flat_map(|&b| &nodes[b])
            .fold(0u64, |set, &a| set | (1 << a));
        let closed = lattice(universe, &edge, options, eh)?;
        stats.lattice_nodes = closed.len();
        let closed_index: HashMap<_, _> = closed
            .iter()
            .enumerate()
            .map(|(i, &s)| (s, i as Label))
            .collect();
        let mut output_sets = vec![0u64; labels.len()];
        for (o, &target) in image.iter().enumerate() {
            output_sets[nodes[o / degree][o % degree]] |=
                1 << nodes[target / degree][target % degree];
        }
        // Meet all occurrence outputs of a and of its edge-diagram successors.
        // Lattice order is REVERSE inclusion, so this meet is closure(union).
        let mapping: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(a, &label)| {
                let set = (0..labels.len())
                    .filter(|&b| edge[a] & !edge[b] == 0)
                    .fold(0u64, |set, b| set | output_sets[b]);
                (label, closed_index[&closure(set, universe, &edge)])
            })
            .collect();
        let diagram: Vec<_> = closed
            .iter()
            .enumerate()
            .flat_map(|(a, &x)| {
                closed.iter().enumerate().filter_map(move |(b, &y)| {
                    if x & y == y {
                        Some((a as Label, b as Label))
                    } else {
                        None
                    }
                })
            })
            .collect();
        let names: Vec<_> = (0..closed.len())
            .map(|a| (a as Label, format!("(L{a})")))
            .collect();
        let map: HashMap<_, _> = mapping.iter().copied().collect();
        // Seed the FULL invariant edge relation, not just mapped input edges.
        // This is an explicit relaxation and makes the actual edge diagram
        // exactly our lattice: x~y iff extent(y) <= N(extent(x)). The relation
        // is upward-closed and coordinatewise meet-closed, so the procedure
        // cannot enlarge it. No extra SAT query is involved.
        let mut expanded = self.clone();
        expanded.active = Constraint {
            lines: tuples
                .iter()
                .map(|t| singleton_line(t.iter().map(|a| map[a])))
                .collect(),
            degree: self.active.degree,
            is_maximized: false,
        };
        let mut lattice_pairs = Vec::new();
        for (a, &x) in closed.iter().enumerate() {
            let neighbors = common_neighbors(x, universe, &edge);
            for (b, &y) in closed.iter().enumerate().skip(a) {
                if y & !neighbors == 0 {
                    lattice_pairs.push(singleton_line([a as Label, b as Label]));
                }
            }
        }
        // Independent check of the initial, port-local relaxation on edges.
        for pair in &pairs {
            let x = closed[map[&pair[0]] as usize];
            let y = closed[map[&pair[1]] as usize];
            if y & !common_neighbors(x, universe, &edge) != 0 {
                return Err("Invalid normalization mapping: an input edge is not preserved".into());
            }
        }
        expanded.passive = Constraint {
            lines: lattice_pairs,
            degree: self.passive.degree,
            is_maximized: false,
        };
        expanded.mapping_label_text = names.clone();
        let identity = (0..closed.len())
            .map(|a| (a as Label, a as Label))
            .collect();
        eh.notify(
            "Normalize: running fixed-point procedure on constructed lattice",
            closed.len(),
            0,
        );
        let (mut problem, _) = expanded
            .fixpoint_onestep(false, &identity, &names, &diagram, None, None, eh)
            .map_err(str::to_string)?;
        check(eh)?;
        problem.compute_triviality(eh);
        if !problem.trivial_sets.as_ref().is_some_and(Vec::is_empty) {
            return Err("The constructed relaxation is trivial. The input may not be a fixed point; this is NOT a nonexistence certificate.".into());
        }
        name_fixed_point_with_mapping(self, &mapping, closed.len(), &mut problem);
        let old_names: HashMap<_, _> = self.mapping_label_text.iter().cloned().collect();
        let new_names: HashMap<_, _> = problem.mapping_label_text.iter().cloned().collect();
        let mut diagram_text =
            String::from("# Mapping from the original input to the construction lattice\n");
        for &(a, b) in &mapping {
            diagram_text += &format!("{} = {}\n", old_names[&a], new_names[&b]);
        }
        diagram_text += "\n# Construction lattice (cover arrows)\n";
        for &(a, b) in &diagram {
            if a != b
                && !(0..closed.len()).any(|c| {
                    c != a as usize
                        && c != b as usize
                        && closed[a as usize] & closed[c] == closed[c]
                        && closed[c] & closed[b as usize] == closed[b as usize]
                })
            {
                diagram_text += &format!("{} -> {}\n", new_names[&a], new_names[&b]);
            }
        }
        // Preserve all construction labels. The ordinary serial::fix_problem
        // prunes auxiliary elements that need not occur in the node constraint.
        problem.compute_diagram(eh);
        if problem.diagram_indirect.as_ref() != Some(&diagram) {
            return Err("Normalization verification failed: the edge diagram differs from the construction lattice".into());
        }
        problem.sort_active_by_strength();
        problem.compute_passive_gen();
        check(eh)?;
        problem.expressions = Some(format!(
            "Lattice normalization\n{} node configurations -> {} core configurations ({} occurrences); {} lattice elements.\nNontriviality checked; the edge diagram equals the construction lattice.\nThe full lattice-compatible edge relation is included.\nTo reuse this lattice: paste the text below into Custom on the ORIGINAL input (Custom need not reproduce the additional edge relaxations).\n\n{}",
            stats.original_configurations, stats.core_configurations, stats.core_occurrences, stats.lattice_nodes, diagram_text));
        eh.notify(
            "Normalize: nontrivial lattice-based relaxation verified",
            closed.len(),
            0,
        );
        Ok(NormalizedFixedPoint {
            problem,
            diagram,
            mapping,
            diagram_text,
            stats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};

    fn example() -> Problem {
        Problem::from_string("A A B\nC C B\nD D E\n\nA AC\nB ED\nD D").unwrap()
    }

    #[test]
    fn example_has_seven_element_lattice_and_nontrivial_relaxation() {
        let source = example();
        let result = source
            .normalize_fixed_point(&Default::default(), &mut EventHandler::null())
            .unwrap();
        assert_eq!(result.stats.core_configurations, 2);
        assert_eq!(result.stats.core_occurrences, 6);
        assert_eq!(result.stats.lattice_nodes, 7);
        assert_eq!(result.problem.trivial_sets, Some(vec![]));
        assert_eq!(result.problem.mapping_label_text.len(), 7);
        assert_eq!(
            result.problem.diagram_indirect.as_ref(),
            Some(&result.diagram)
        );
        let old: HashMap<_, _> = source
            .mapping_label_text
            .iter()
            .map(|(a, s)| (s.as_str(), *a))
            .collect();
        let map: HashMap<_, _> = result.mapping.iter().copied().collect();
        assert_eq!(map[&old["A"]], map[&old["C"]]);
        assert!(result
            .problem
            .mapping_label_text
            .iter()
            .any(|(_, s)| s == "(A=C)"));
        let order: BTreeSet<_> = result.diagram.iter().copied().collect();
        for a in 0..7 {
            for b in 0..7 {
                let lower: Vec<_> = (0..7)
                    .filter(|&c| order.contains(&(c, a)) && order.contains(&(c, b)))
                    .collect();
                assert_eq!(
                    lower
                        .iter()
                        .filter(|&&m| lower.iter().all(|&x| order.contains(&(x, m))))
                        .count(),
                    1
                );
                let upper: Vec<_> = (0..7)
                    .filter(|&c| order.contains(&(a, c)) && order.contains(&(b, c)))
                    .collect();
                assert_eq!(
                    upper
                        .iter()
                        .filter(|&&m| upper.iter().all(|&x| order.contains(&(m, x))))
                        .count(),
                    1
                );
            }
        }
        // Independently replay using the public custom-diagram parser.
        let mut source = source;
        source.compute_diagram(&mut EventHandler::null());
        let (replayed, _, _) = source
            .fixpoint_custom(
                result.diagram_text.clone(),
                false,
                &mut EventHandler::null(),
            )
            .unwrap();
        let mut replayed = replayed;
        replayed.compute_triviality(&mut EventHandler::null());
        assert_eq!(replayed.trivial_sets, Some(vec![]));
    }

    #[test]
    fn repeated_labels_do_not_merge_occurrences_inside_a_node() {
        let nodes = vec![vec![0, 0, 1], vec![0, 1, 1]];
        let edge = vec![1, 2];
        let options = NormalizationOptions::default();
        // Check that any satisfying solution really matches all three distinct
        // occurrences, even when several share the same original label.
        let result = core_map(
            &nodes,
            &edge,
            &[0, 1],
            0,
            &options,
            &mut EventHandler::null(),
        )
        .unwrap();
        if let Some(image) = result {
            validate_core_map(&nodes, &edge, &[0, 1], &[1], &image).unwrap();
        }
        let invalid = vec![3, 3, 4, 3, 4, 5];
        assert!(validate_core_map(&nodes, &edge, &[0, 1], &[1], &invalid).is_err());
    }

    #[test]
    fn core_sat_agrees_with_brute_force_on_all_three_label_edge_relations() {
        let nodes = vec![vec![0, 0], vec![1, 2], vec![0, 2]];
        let options = NormalizationOptions::default();
        for bits in 0..64 {
            let mut edge = vec![0u64; 3];
            let mut bit = 0;
            for a in 0..3 {
                for b in a..3 {
                    if bits & (1 << bit) != 0 {
                        edge[a] |= 1 << b;
                        edge[b] |= 1 << a;
                    }
                    bit += 1;
                }
            }
            for omitted in 0..3 {
                let targets: Vec<_> = (0..3).filter(|&b| b != omitted).collect();
                // Each source block chooses one of two targets and one of its
                // two port permutations. Enumerate all 4^3 possibilities.
                let brute = (0..64).any(|mut code| {
                    let mut image = Vec::new();
                    for _ in 0..3 {
                        let target = targets[(code % 4) / 2];
                        let flip = code % 2;
                        image.extend([target * 2 + flip, target * 2 + 1 - flip]);
                        code /= 4;
                    }
                    (0..6).all(|a| {
                        (0..6).all(|b| {
                            let label = |i: usize| nodes[i / 2][i % 2];
                            edge[label(a)] & (1 << label(b)) == 0
                                || edge[label(image[a])] & (1 << label(image[b])) != 0
                        })
                    })
                });
                let sat = core_map(
                    &nodes,
                    &edge,
                    &[0, 1, 2],
                    omitted,
                    &options,
                    &mut EventHandler::null(),
                )
                .unwrap();
                assert_eq!(
                    sat.is_some(),
                    brute,
                    "edge mask {bits}, omitted block {omitted}"
                );
            }
        }
    }

    #[test]
    fn intersection_lattice_matches_all_double_neighborhood_closed_sets() {
        for bits in 0..64 {
            let mut edge = vec![0u64; 3];
            let mut bit = 0;
            for a in 0..3 {
                for b in a..3 {
                    if bits & (1 << bit) != 0 {
                        edge[a] |= 1 << b;
                        edge[b] |= 1 << a;
                    }
                    bit += 1;
                }
            }
            for universe in 1..8 {
                let expected: Vec<_> = (0..8)
                    .filter(|&s| s & !universe == 0 && closure(s, universe, &edge) == s)
                    .collect();
                let actual = lattice(
                    universe,
                    &edge,
                    &Default::default(),
                    &mut EventHandler::null(),
                )
                .unwrap();
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn example_is_a_relaxation_and_a_weak_fixed_point_after_two_real_speedups() {
        let source = example();
        let mut eh = EventHandler::null();
        let result = source
            .normalize_fixed_point(&Default::default(), &mut eh)
            .unwrap();
        let after = result.problem.speedup(&mut eh).speedup(&mut eh);
        // Use the independent existing zero-round SAT checker, not the core
        // encoding. Its input here is live (no passive-only label identity bug).
        let mut target = result.problem.clone();
        target.compute_triviality_with_input(source, true);
        assert_eq!(target.is_trivial_with_input, Some(true));
        let mut target = result.problem;
        target.compute_triviality_with_input(after, true);
        assert_eq!(target.is_trivial_with_input, Some(true));
    }

    #[test]
    fn serial_request_keeps_auxiliary_lattice_elements() {
        use crate::serial::{request_json, Request, Response};
        use std::sync::Mutex;
        let request = serde_json::to_string(&Request::FixpointNormalize(example())).unwrap();
        let responses = Mutex::new(Vec::new());
        request_json(&request, |json, _| {
            responses
                .lock()
                .unwrap()
                .push(serde_json::from_str::<Response>(&json).unwrap())
        });
        let responses = responses.into_inner().unwrap();
        let problems: Vec<_> = responses
            .iter()
            .filter_map(|r| match r {
                Response::P(p) => Some(p),
                Response::E(message) => panic!("{message}"),
                _ => None,
            })
            .collect();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].mapping_label_text.len(), 7);
        assert_eq!(problems[0].trivial_sets, Some(vec![]));
        assert!(problems[0]
            .expressions
            .as_ref()
            .unwrap()
            .contains("edge diagram equals the construction lattice"));
        assert!(matches!(responses.last(), Some(Response::Done)));
    }

    #[test]
    fn nonfixed_input_may_fail_without_claiming_nonexistence() {
        let source = Problem::from_string("A B\nA C\nB C\n\nA A\nB B\nC C").unwrap();
        let error = source
            .normalize_fixed_point(&Default::default(), &mut EventHandler::null())
            .unwrap_err();
        assert!(error.contains("trivial"));
        assert!(error.contains("NOT a nonexistence certificate"));
    }

    #[test]
    fn bounded_and_cancelled_runs_are_inconclusive() {
        let mut options = NormalizationOptions::default();
        options.max_lattice_nodes = 2;
        assert!(example()
            .normalize_fixed_point(&options, &mut EventHandler::null())
            .unwrap_err()
            .contains("inconclusive"));
        options = NormalizationOptions::default();
        options.max_sat_clauses = 0;
        assert!(example()
            .normalize_fixed_point(&options, &mut EventHandler::null())
            .unwrap_err()
            .contains("inconclusive"));
        let mut eh = EventHandler::null().with_cancellation(Arc::new(AtomicBool::new(true)));
        assert!(example()
            .normalize_fixed_point(&Default::default(), &mut eh)
            .unwrap_err()
            .contains("cancelled"));
    }

    #[test]
    fn rejects_trivial_or_unsupported_inputs() {
        let mut trivial = Problem::from_string("A A\n\nA A").unwrap();
        assert!(trivial
            .normalize_fixed_point(&Default::default(), &mut EventHandler::null())
            .unwrap_err()
            .contains("zero-round"));
        trivial.orientation_given = Some(1);
        assert!(trivial
            .normalize_fixed_point(&Default::default(), &mut EventHandler::null())
            .unwrap_err()
            .contains("unoriented"));
        let hyper = Problem::from_string("A A\n\nA A A").unwrap();
        assert!(hyper
            .normalize_fixed_point(&Default::default(), &mut EventHandler::null())
            .unwrap_err()
            .contains("edge degree 2"));
    }
}
