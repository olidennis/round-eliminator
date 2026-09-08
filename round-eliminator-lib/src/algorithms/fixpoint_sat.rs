//! Native finite-lattice synthesis. Candidates contain only ordinary lattice
//! elements and a (possibly noninjective) interpretation of the input labels.
//! See docs/fixpoint-sat.md for the encoding and failure-certificate argument.

use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Assignment, Lit, TernaryVal};
use rustsat_minisat::{core::Minisat, Limit};

use crate::{
    constraint::Constraint,
    group::{Group, Label},
    line::{Degree, Line},
    problem::Problem,
};

use super::{
    event::EventHandler,
    fixpoint::{Tracking, TreeNode},
    maximize::Operation,
    nofixpoint::NonexistenceOracle,
};

type Term = TreeNode<Label>;
type Obstruction = Vec<(Term, Term)>;

mod game;
mod proof;
mod search;

pub use proof::instance as certificate_cnf;
pub use proof::{CertificateSearchOptions, CertificateSearchOutcome};
use search::SearchControl;

#[derive(Clone, Debug)]
pub struct SatSearchOptions {
    pub min_nodes: usize,
    /// None searches increasing sizes without an upper bound.
    pub max_nodes: Option<usize>,
    /// Total candidate budget across all sizes. Reaching it is inconclusive.
    pub max_candidates: Option<usize>,
    /// Per SAT call. Reaching this limit is inconclusive, never UNSAT.
    pub conflict_limit: Option<u32>,
    pub generalize: bool,
    /// Reject candidates with the witness-producing tree game. A winning
    /// diagram is still materialized and checked by the full procedure.
    pub use_game: bool,
    /// Reuse the symbolic loop's unbounded nonexistence test.
    pub check_nonexistence: bool,
}

impl Default for SatSearchOptions {
    fn default() -> Self {
        Self {
            min_nodes: 1,
            max_nodes: None,
            max_candidates: None,
            conflict_limit: None,
            generalize: true,
            use_game: true,
            check_nonexistence: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SatSearchStats {
    pub candidates: usize,
    pub game_checks: usize,
    pub game_positions: usize,
    pub game_moves: usize,
    pub full_constructions: usize,
    pub generalized_blockers: usize,
    pub exact_blockers: usize,
    pub exhausted_sizes: Vec<usize>,
    pub certificate_steps: usize,
    pub certificate_shared_lines: usize,
}

#[derive(Debug)]
pub struct SatFixedPoint {
    pub problem: Problem,
    pub nodes: usize,
    /// Reflexive, transitive order of the successful lattice.
    pub diagram: Vec<(Label, Label)>,
    pub mapping: Vec<(Label, Label)>,
    /// Mapping and edges in the existing custom-diagram input format.
    pub diagram_text: String,
    pub stats: SatSearchStats,
}

#[derive(Debug)]
pub enum SatSearchOutcome {
    Found(SatFixedPoint),
    /// An all-size certificate from the existing symbolic proof oracle.
    NoFixedPoint {
        certificate: String,
        stats: SatSearchStats,
    },
    /// Only the stated finite interval of sizes has been ruled out.
    Exhausted {
        min_nodes: usize,
        max_nodes: usize,
        stats: SatSearchStats,
    },
    Inconclusive {
        nodes: usize,
        stats: SatSearchStats,
    },
}

struct Encoding {
    solver: Minisat,
    next_var: u32,
    truth: Lit,
    nodes: usize,
    order: Vec<Vec<Lit>>,
    join: Vec<Vec<Vec<Lit>>>,
    meet: Vec<Vec<Vec<Lit>>>,
    labels: Vec<Label>,
    mapping: HashMap<Label, Vec<Lit>>,
    terms: HashMap<Term, Vec<Lit>>,
    inequalities: HashMap<(Term, Term), Lit>,
    cancellation: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Encoding {
    fn literal(&mut self) -> Lit {
        let lit = Lit::new(self.next_var, false);
        self.next_var = self
            .next_var
            .checked_add(1)
            .expect("SAT variable limit exceeded");
        lit
    }

    fn clause(&mut self, literals: impl IntoIterator<Item = Lit>) -> Result<(), String> {
        search::check_cancelled(self.cancellation.as_deref())?;
        let mut literals: Vec<_> = literals.into_iter().collect();
        if literals.contains(&self.truth) {
            return Ok(());
        }
        literals.retain(|l| *l != !self.truth);
        literals.sort_unstable();
        literals.dedup();
        if literals
            .iter()
            .any(|l| literals.binary_search(&!*l).is_ok())
        {
            return Ok(());
        }
        self.solver
            .add_clause(literals.into_iter().collect())
            .map_err(|e| e.to_string())
    }

    fn one_hot(&mut self, values: &[Lit]) -> Result<(), String> {
        self.clause(values.iter().copied())?;
        for i in 0..values.len() {
            for j in i + 1..values.len() {
                self.clause([!values[i], !values[j]])?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn new(problem: &Problem, nodes: usize) -> Result<Self, String> {
        Self::new_cancellable(problem, nodes, None)
    }

    fn new_cancellable(
        problem: &Problem,
        nodes: usize,
        cancellation: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<Self, String> {
        let truth = Lit::new(0, false);
        let mut solver = Minisat::default();
        solver
            .add_clause([truth].into_iter().collect())
            .map_err(|e| e.to_string())?;
        let mut labels = problem.labels();
        labels.sort_unstable();
        let mut enc = Self {
            solver,
            next_var: 1,
            truth,
            nodes,
            order: vec![vec![!truth; nodes]; nodes],
            join: vec![vec![vec![!truth; nodes]; nodes]; nodes],
            meet: vec![vec![vec![!truth; nodes]; nodes]; nodes],
            labels,
            mapping: HashMap::new(),
            terms: HashMap::new(),
            inequalities: HashMap::new(),
            cancellation,
        };

        // Every finite lattice can be numbered in a linear extension, with
        // bottom 0 and top N-1. Label interpretations remain solver variables.
        for a in 0..nodes {
            for b in a..nodes {
                enc.order[a][b] = if a == b || a == 0 || b == nodes - 1 {
                    truth
                } else {
                    enc.literal()
                };
            }
        }
        for a in 0..nodes {
            for b in a + 1..nodes {
                for c in b + 1..nodes {
                    enc.clause([!enc.order[a][b], !enc.order[b][c], enc.order[a][c]])?;
                }
            }
        }
        for a in 0..nodes {
            enc.join[a][a][a] = truth;
            enc.meet[a][a][a] = truth;
            for b in a + 1..nodes {
                let mut joins = vec![!truth; nodes];
                let mut meets = vec![!truth; nodes];
                for k in b..nodes {
                    joins[k] = enc.literal();
                }
                for k in 0..=a {
                    meets[k] = enc.literal();
                }
                enc.one_hot(&joins[b..])?;
                enc.one_hot(&meets[..=a])?;
                for k in b..nodes {
                    enc.clause([!joins[k], enc.order[a][k]])?;
                    enc.clause([!joins[k], enc.order[b][k]])?;
                    for z in b..nodes {
                        enc.clause([
                            !joins[k],
                            !enc.order[a][z],
                            !enc.order[b][z],
                            enc.order[k][z],
                        ])?;
                    }
                }
                for k in 0..=a {
                    enc.clause([!meets[k], enc.order[k][a]])?;
                    enc.clause([!meets[k], enc.order[k][b]])?;
                    for z in 0..=a {
                        enc.clause([
                            !meets[k],
                            !enc.order[z][a],
                            !enc.order[z][b],
                            enc.order[z][k],
                        ])?;
                    }
                }
                enc.join[a][b] = joins.clone();
                enc.join[b][a] = joins;
                enc.meet[a][b] = meets.clone();
                enc.meet[b][a] = meets;
            }
        }
        for label in enc.labels.clone() {
            let row: Vec<_> = (0..nodes).map(|_| enc.literal()).collect();
            enc.one_hot(&row)?;
            enc.mapping.insert(label, row);
        }
        for &(a, b) in problem.diagram_indirect.as_ref().unwrap() {
            if a == b {
                continue;
            }
            for i in 0..nodes {
                for j in 0..nodes {
                    enc.clause([!enc.mapping[&a][i], !enc.mapping[&b][j], enc.order[i][j]])?;
                }
            }
        }
        Ok(enc)
    }

    fn term(&mut self, term: &Term) -> Result<Vec<Lit>, String> {
        if let Some(row) = self.terms.get(term) {
            return Ok(row.clone());
        }
        let row = match term {
            Term::Terminal(label) => self.mapping[label].clone(),
            Term::Expr(a, b, operation) => {
                let a = self.term(a)?;
                let b = self.term(b)?;
                let result: Vec<_> = (0..self.nodes).map(|_| self.literal()).collect();
                self.one_hot(&result)?;
                for i in 0..self.nodes {
                    for j in 0..self.nodes {
                        for k in 0..self.nodes {
                            let op = match operation {
                                Operation::Union => self.join[i][j][k],
                                Operation::Intersection => self.meet[i][j][k],
                            };
                            self.clause([!a[i], !b[j], !op, result[k]])?;
                        }
                    }
                }
                result
            }
        };
        self.terms.insert(term.clone(), row.clone());
        Ok(row)
    }

    fn inequality(&mut self, a: &Term, b: &Term) -> Result<Lit, String> {
        let key = (a.clone(), b.clone());
        if let Some(&lit) = self.inequalities.get(&key) {
            return Ok(lit);
        }
        let left = self.term(a)?;
        let right = self.term(b)?;
        let result = self.literal();
        for i in 0..self.nodes {
            for j in 0..self.nodes {
                self.clause([!left[i], !right[j], !result, self.order[i][j]])?;
                self.clause([!left[i], !right[j], result, !self.order[i][j]])?;
            }
        }
        self.inequalities.insert(key, result);
        Ok(result)
    }

    fn block(&mut self, obstruction: &Obstruction) -> Result<(), String> {
        let mut clause = Vec::new();
        for (a, b) in obstruction {
            clause.push(!self.inequality(a, b)?);
        }
        self.clause(clause)
    }

    fn block_exact(&mut self, candidate: &Candidate) -> Result<(), String> {
        let mut clause = Vec::new();
        for a in 0..self.nodes {
            for b in a + 1..self.nodes {
                clause.push(if candidate.order[a][b] {
                    !self.order[a][b]
                } else {
                    self.order[a][b]
                });
            }
        }
        for &label in &self.labels {
            clause.push(!self.mapping[&label][candidate.mapping[&label]]);
        }
        self.clause(clause)
    }

    fn candidate(&self, assignment: &Assignment) -> Result<Candidate, String> {
        let value = |lit| assignment.lit_value(lit) == TernaryVal::True;
        let selected = |row: &[Lit]| {
            row.iter()
                .position(|&l| value(l))
                .ok_or_else(|| "Incomplete SAT assignment".to_string())
        };
        let order = self
            .order
            .iter()
            .map(|r| r.iter().map(|&l| value(l)).collect())
            .collect();
        let mut mapping = HashMap::new();
        for (&label, row) in &self.mapping {
            mapping.insert(label, selected(row)?);
        }
        let mut join = vec![vec![0; self.nodes]; self.nodes];
        let mut meet = join.clone();
        for a in 0..self.nodes {
            for b in 0..self.nodes {
                join[a][b] = selected(&self.join[a][b])?;
                meet[a][b] = selected(&self.meet[a][b])?;
            }
        }
        Ok(Candidate {
            order,
            mapping,
            join,
            meet,
        })
    }
}

struct Candidate {
    order: Vec<Vec<bool>>,
    mapping: HashMap<Label, usize>,
    join: Vec<Vec<usize>>,
    meet: Vec<Vec<usize>>,
}

impl Candidate {
    fn eval(&self, term: &Term) -> usize {
        match term {
            Term::Terminal(label) => self.mapping[label],
            Term::Expr(a, b, Operation::Union) => self.join[self.eval(a)][self.eval(b)],
            Term::Expr(a, b, Operation::Intersection) => self.meet[self.eval(a)][self.eval(b)],
        }
    }

    fn satisfies(&self, obstruction: &Obstruction) -> bool {
        obstruction
            .iter()
            .all(|(a, b)| self.order[self.eval(a)][self.eval(b)])
    }

    fn diagram(&self) -> Vec<(Label, Label)> {
        self.order
            .iter()
            .enumerate()
            .flat_map(|(a, row)| {
                row.iter()
                    .enumerate()
                    .filter_map(move |(b, &le)| le.then_some((a as Label, b as Label)))
            })
            .collect()
    }

    fn label_mapping(&self) -> Vec<(Label, Label)> {
        let mut mapping: Vec<_> = self
            .mapping
            .iter()
            .map(|(&a, &b)| (a, b as Label))
            .collect();
        mapping.sort_unstable();
        mapping
    }
}

fn expanded(line: &Line) -> Vec<Label> {
    line.parts
        .iter()
        .flat_map(|p| std::iter::repeat(p.group.first()).take(p.gtype.value()))
        .collect()
}

/// Tracks actual input occurrences, not an inverse label map: several original
/// labels may have the same value in a candidate. Arbitrarily choosing one
/// original label for a merged value would make proof certificates unsound.
struct Provenance<'a> {
    tracking: &'a DashMap<Line, Tracking>,
    memo: HashMap<Line, Vec<Term>>,
    reverse: bool,
}

impl<'a> Provenance<'a> {
    fn new(
        original: &Constraint,
        candidate: &Candidate,
        tracking: &'a DashMap<Line, Tracking>,
        reverse: bool,
    ) -> Self {
        let mut memo = HashMap::new();
        let mut choices = original.all_choices(true);
        choices.sort();
        for line in choices {
            let mut mapped = line.clone();
            let mut originals: HashMap<Label, Vec<Label>> = HashMap::new();
            for label in expanded(&line) {
                originals
                    .entry(candidate.mapping[&label] as Label)
                    .or_default()
                    .push(label);
            }
            for part in &mut mapped.parts {
                part.group = Group::from(vec![candidate.mapping[&part.group.first()] as Label]);
            }
            mapped.normalize();
            let terms = mapped
                .parts
                .iter()
                .flat_map(|part| {
                    originals[&part.group.first()]
                        .iter()
                        .copied()
                        .map(Term::Terminal)
                })
                .collect();
            memo.entry(mapped).or_insert(terms);
        }
        Self {
            tracking,
            memo,
            reverse,
        }
    }

    fn terms(&mut self, line: &Line) -> Result<Vec<Term>, String> {
        if let Some(terms) = self.memo.get(line) {
            return Ok(terms.clone());
        }
        let (left, right, before, normalization, operations) = self
            .tracking
            .get(line)
            .ok_or_else(|| "Missing fixed-point derivation".to_string())?
            .clone();
        let left_terms = self.terms(&left)?;
        let right_terms = self.terms(&right)?;
        let offsets = |line: &Line| {
            let mut offset = 0;
            line.parts
                .iter()
                .map(|p| {
                    let start = offset;
                    offset += p.gtype.value();
                    start
                })
                .collect::<Vec<_>>()
        };
        let left_offsets = offsets(&left);
        let right_offsets = offsets(&right);
        let mut used_left = vec![0; left.parts.len()];
        let mut used_right = vec![0; right.parts.len()];
        let mut result = Vec::new();
        for index in normalization.iter().flatten().copied() {
            let (a, b, mut operation) = operations[index];
            if self.reverse {
                operation = match operation {
                    Operation::Union => Operation::Intersection,
                    Operation::Intersection => Operation::Union,
                };
            }
            for _ in 0..before.parts[index].gtype.value() {
                let x = left_terms
                    .get(left_offsets[a] + used_left[a])
                    .ok_or_else(|| "Invalid left derivation position".to_string())?;
                let y = right_terms
                    .get(right_offsets[b] + used_right[b])
                    .ok_or_else(|| "Invalid right derivation position".to_string())?;
                result.push(Term::Expr(
                    Box::new(x.clone()),
                    Box::new(y.clone()),
                    operation,
                ));
                used_left[a] += 1;
                used_right[b] += 1;
            }
        }
        if result.len() != expanded(line).len() {
            return Err("Invalid derivation degree".to_string());
        }
        self.memo.insert(line.clone(), result.clone());
        Ok(result)
    }
}

struct Failure {
    active_terms: Vec<Term>,
    other_active_terms: Vec<Vec<Term>>,
    obstruction: Obstruction,
}

/// Presentation only: preserve original names on their images and explicitly
/// name mergers. Do not use mapping_label_oldlabels: a lattice node is not a
/// speedup label/set, and "rename by generators" would be misleading here.
fn name_fixed_point(original: &Problem, candidate: &Candidate, result: &mut Problem) {
    let original_names: HashMap<_, _> = original.mapping_label_text.iter().cloned().collect();
    let mapping = candidate.label_mapping();
    let mut preimages = vec![Vec::new(); candidate.order.len()];
    for &(a, b) in &mapping {
        preimages[b as usize].push(original_names[&a].clone());
    }
    for labels in &mut preimages {
        labels.sort();
    }
    let unwrapped = |name: &str| {
        name.strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(name)
            .to_string()
    };
    // Reserve every original name, including labels inside mergers. For
    // example, a merger of A and B must not impersonate an original (A=B).
    let mut reserved: HashSet<_> = original_names.values().cloned().collect();
    let mut names = Vec::new();
    for (node, labels) in preimages.iter().enumerate() {
        let name = if labels.len() == 1 {
            labels[0].clone()
        } else {
            let body = if labels.is_empty() {
                format!("FP{node}")
            } else {
                labels
                    .iter()
                    .map(|s| unwrapped(s))
                    .collect::<Vec<_>>()
                    .join("=")
            };
            let mut name = format!("({body})");
            let mut suffix = 0;
            while reserved.contains(&name) {
                name = format!("({body}_FP{node}_{suffix})");
                suffix += 1;
            }
            reserved.insert(name.clone());
            name
        };
        names.push((node as Label, name));
    }
    result.mapping_label_text = names;
    result.mapping_oldlabel_text = Some(
        mapping
            .iter()
            .map(|&(a, _)| (a, original_names[&a].clone()))
            .collect(),
    );
    result.mapping_oldlabel_labels = Some(mapping.into_iter().map(|(a, b)| (a, vec![b])).collect());
}

/// An active derivation A and, for each ordered pair of positions of A, a
/// passive derivation (p,q) with p <= A[i], q <= A[j]. These inequalities
/// suffice for triviality in any lattice. Passive derivations use dual
/// operations, since procedure() runs on the reversed diagram on that side.
fn failure(
    original: &Problem,
    result: &Problem,
    passive: &Constraint,
    candidate: &Candidate,
    active_tracking: &DashMap<Line, Tracking>,
    passive_tracking: &DashMap<Line, Tracking>,
) -> Result<Failure, String> {
    let trivial = result.trivial_sets.as_ref().unwrap();
    let mut lines: Vec<_> = result
        .active
        .lines
        .iter()
        .filter(|l| {
            trivial
                .iter()
                .any(|set| l.parts.iter().all(|p| set.contains(&p.group.first())))
        })
        .collect();
    lines.sort();
    let line = lines
        .first()
        .ok_or_else(|| "Missing trivial active line".to_string())?;
    let mut active_provenance =
        Provenance::new(&original.active, candidate, active_tracking, false);
    let active_terms = active_provenance.terms(line)?;
    let active_values = expanded(line);
    if active_terms
        .iter()
        .map(|t| candidate.eval(t) as Label)
        .collect::<Vec<_>>()
        != active_values
    {
        return Err("Active derivation does not evaluate to its recorded line".to_string());
    }
    let mut other_active_terms = Vec::new();
    for line in lines.iter().skip(1) {
        let terms = active_provenance.terms(line)?;
        if terms
            .iter()
            .map(|t| candidate.eval(t) as Label)
            .collect::<Vec<_>>()
            != expanded(line)
        {
            return Err("Active derivation does not evaluate to its recorded line".to_string());
        }
        other_active_terms.push(terms);
    }

    failure_from_terms(
        original,
        active_terms,
        other_active_terms,
        passive,
        candidate,
        passive_tracking,
    )
}

fn failure_from_terms(
    original: &Problem,
    active_terms: Vec<Term>,
    other_active_terms: Vec<Vec<Term>>,
    passive: &Constraint,
    candidate: &Candidate,
    passive_tracking: &DashMap<Line, Tracking>,
) -> Result<Failure, String> {
    let active_values: Vec<_> = active_terms
        .iter()
        .map(|t| candidate.eval(t) as Label)
        .collect();
    let mut passive_provenance =
        Provenance::new(&original.passive, candidate, passive_tracking, true);
    let mut passive_lines: Vec<_> = passive.lines.iter().collect();
    passive_lines.sort();
    let mut obstruction = HashSet::new();
    for (i, &a) in active_values.iter().enumerate() {
        for (j, &b) in active_values.iter().enumerate() {
            let mut witness = None;
            for line in &passive_lines {
                let values = expanded(line);
                for (x, y) in [(0, 1), (1, 0)] {
                    if candidate.order[values[x] as usize][a as usize]
                        && candidate.order[values[y] as usize][b as usize]
                    {
                        witness = Some((*line, x, y));
                        break;
                    }
                }
                if witness.is_some() {
                    break;
                }
            }
            let (passive_line, x, y) =
                witness.ok_or_else(|| "Missing passive triviality witness".to_string())?;
            let terms = passive_provenance.terms(passive_line)?;
            if terms
                .iter()
                .map(|t| candidate.eval(t) as Label)
                .collect::<Vec<_>>()
                != expanded(passive_line)
            {
                return Err("Passive derivation does not evaluate to its recorded line".to_string());
            }
            obstruction.insert((terms[x].clone(), active_terms[i].clone()));
            obstruction.insert((terms[y].clone(), active_terms[j].clone()));
        }
    }
    let mut obstruction: Vec<_> = obstruction.into_iter().collect();
    obstruction.sort();
    if !candidate.satisfies(&obstruction) {
        return Err("Failure certificate does not exclude its candidate".to_string());
    }
    Ok(Failure {
        active_terms,
        other_active_terms,
        obstruction,
    })
}

impl Problem {
    /// Search all lattices in the requested size interval that respect the
    /// supplied input order (computed if absent), including label mergers.
    /// By default the tree game rejects candidates; success is materialized
    /// and checked by the existing fixed-point/triviality procedures.
    /// Finite exhaustion, budget limits, and global nonexistence are distinct.
    pub fn fixpoint_sat(
        &self,
        options: &SatSearchOptions,
        eh: &mut EventHandler,
    ) -> Result<SatSearchOutcome, String> {
        self.fixpoint_sat_worker(options, eh, None, None, None)
    }

    fn fixpoint_sat_worker(
        &self,
        options: &SatSearchOptions,
        eh: &mut EventHandler,
        control: Option<&SearchControl>,
        hints: Option<&std::sync::mpsc::SyncSender<Vec<Term>>>,
        derivations: Option<&std::sync::mpsc::Sender<proof::guided::Derivation>>,
    ) -> Result<SatSearchOutcome, String> {
        if options.min_nodes == 0 || options.max_nodes.is_some_and(|n| n < options.min_nodes) {
            return Err("SAT diagram search requires 1 <= min_nodes <= max_nodes".to_string());
        }
        if !matches!(self.active.degree, Degree::Finite(n) if n > 0)
            || self.passive.degree != Degree::Finite(2)
        {
            return Err(
                "SAT diagram search requires finite positive active degree and passive degree 2"
                    .to_string(),
            );
        }
        let mut original = self.clone();
        if original.diagram_indirect.is_none() {
            original.compute_diagram(eh);
        }
        let mut oracle = options
            .check_nonexistence
            .then(|| NonexistenceOracle::new(&original));
        let mut stats = SatSearchStats::default();
        let mut obstructions: Vec<Obstruction> = Vec::new();
        let mut nodes = options.min_nodes;
        loop {
            search::check_event(eh)?;
            if options
                .max_candidates
                .is_some_and(|limit| stats.candidates >= limit)
            {
                return Ok(SatSearchOutcome::Inconclusive { nodes, stats });
            }
            if nodes > Label::MAX as usize {
                return Err("SAT diagram size exceeds the label representation".to_string());
            }
            eh.notify(
                "SAT: encoding lattice",
                nodes,
                options.max_nodes.unwrap_or(0),
            );
            let mut encoding =
                Encoding::new_cancellable(&original, nodes, eh.cancellation_token())?;
            for obstruction in &obstructions {
                encoding.block(obstruction)?;
            }
            loop {
                if let Some(control) = control {
                    control.record_diagram_stats(&stats);
                }
                search::check_event(eh)?;
                if options
                    .max_candidates
                    .is_some_and(|limit| stats.candidates >= limit)
                {
                    return Ok(SatSearchOutcome::Inconclusive { nodes, stats });
                }
                eh.notify(
                    format!("SAT: searching {nodes}-node diagrams"),
                    stats.candidates,
                    options.max_candidates.unwrap_or(0),
                );
                encoding.solver.set_limit(
                    options
                        .conflict_limit
                        .map_or(Limit::None, |n| Limit::Conflicts(n as i64)),
                );
                let solved = if let Some(control) = control {
                    control.solve(0, &mut encoding.solver, None)?
                } else {
                    encoding.solver.solve().map_err(|e| e.to_string())?
                };
                search::check_event(eh)?;
                match solved {
                    SolverResult::Unsat => break,
                    SolverResult::Interrupted => {
                        return Ok(SatSearchOutcome::Inconclusive { nodes, stats })
                    }
                    SolverResult::Sat => {}
                }
                let assignment = encoding.solver.full_solution().map_err(|e| e.to_string())?;
                let candidate = encoding.candidate(&assignment)?;
                stats.candidates += 1;
                let diagram = candidate.diagram();
                let mapping = candidate.label_mapping();
                let names: Vec<_> = (0..nodes)
                    .map(|n| (n as Label, format!("(SAT{n})")))
                    .collect();
                let active_tracking = DashMap::new();
                let passive_tracking = DashMap::new();
                let track = options.generalize || oracle.is_some() || hints.is_some();
                let mut materialized = None;
                let (trivial, failure) = if options.use_game {
                    stats.game_checks += 1;
                    if let Some(control) = control {
                        control.record_diagram_stats(&stats);
                    }
                    let checked = game::check(
                        &original,
                        &candidate,
                        track.then_some(&passive_tracking),
                        oracle.is_some(),
                        eh,
                    )?;
                    search::check_event(eh)?;
                    stats.game_positions += checked.positions;
                    stats.game_moves += checked.moves;
                    if let Some(derivations) = derivations {
                        if !checked.derivation.steps.is_empty() {
                            let _ = derivations.send(checked.derivation);
                        }
                    }
                    let mut witnesses = checked.active_terms.into_iter();
                    if let Some(terms) = witnesses.next() {
                        let failure = if track {
                            Some(failure_from_terms(
                                &original,
                                terms,
                                witnesses.collect(),
                                &checked.passive,
                                &candidate,
                                &passive_tracking,
                            )?)
                        } else {
                            None
                        };
                        (true, failure)
                    } else {
                        (false, None)
                    }
                } else {
                    stats.full_constructions += 1;
                    if let Some(control) = control {
                        control.record_diagram_stats(&stats);
                    }
                    let (mut problem, passive) = original
                        .fixpoint_onestep(
                            false,
                            &mapping,
                            &names,
                            &diagram,
                            track.then_some(&active_tracking),
                            track.then_some(&passive_tracking),
                            eh,
                        )
                        .map_err(|e| format!("Invalid SAT candidate: {e}"))?;
                    problem.compute_triviality(eh);
                    search::check_event(eh)?;
                    let trivial = !problem.trivial_sets.as_ref().unwrap().is_empty();
                    let failure = if trivial && track {
                        Some(failure(
                            &original,
                            &problem,
                            &passive,
                            &candidate,
                            &active_tracking,
                            &passive_tracking,
                        )?)
                    } else {
                        None
                    };
                    materialized = Some(problem);
                    (trivial, failure)
                };
                if let Some(control) = control {
                    control.record_diagram_stats(&stats);
                }
                if !trivial {
                    let mut problem = if let Some(problem) = materialized {
                        problem
                    } else {
                        eh.notify("SAT: materializing successful diagram", nodes, 0);
                        stats.full_constructions += 1;
                        if let Some(control) = control {
                            control.record_diagram_stats(&stats);
                        }
                        let (mut problem, _) = original
                            .fixpoint_onestep(false, &mapping, &names, &diagram, None, None, eh)
                            .map_err(|e| format!("Invalid SAT candidate: {e}"))?;
                        problem.compute_triviality(eh);
                        search::check_event(eh)?;
                        if !problem.trivial_sets.as_ref().unwrap().is_empty() {
                            return Err(
                                "Tree game disagrees with full fixed-point procedure".to_string()
                            );
                        }
                        problem
                    };
                    name_fixed_point(&original, &candidate, &mut problem);
                    let names: HashMap<_, _> = problem.mapping_label_text.iter().cloned().collect();
                    let original_names: HashMap<_, _> =
                        original.mapping_label_text.iter().cloned().collect();
                    let mut diagram_text = String::from("# original-label mapping\n");
                    for &(a, b) in &mapping {
                        diagram_text.push_str(&format!("{} = {}\n", original_names[&a], names[&b]));
                    }
                    diagram_text.push_str("# lattice order\n");
                    for &(a, b) in &diagram {
                        diagram_text.push_str(&format!("{} -> {}\n", names[&a], names[&b]));
                    }
                    return Ok(SatSearchOutcome::Found(SatFixedPoint {
                        problem,
                        nodes,
                        diagram,
                        mapping,
                        diagram_text,
                        stats,
                    }));
                }

                if !track {
                    encoding.block_exact(&candidate)?;
                    stats.exact_blockers += 1;
                    continue;
                }
                let failure =
                    failure.ok_or_else(|| "Missing candidate failure witness".to_string())?;
                // These are whole, valid active derivations, not arbitrary
                // independently selected expressions. Hints never block SAT.
                if let Some(hints) = hints {
                    for terms in
                        std::iter::once(&failure.active_terms).chain(&failure.other_active_terms)
                    {
                        let _ = hints.try_send(terms.clone());
                    }
                }
                if let Some(oracle) = &mut oracle {
                    for terms in
                        std::iter::once(&failure.active_terms).chain(&failure.other_active_terms)
                    {
                        if let Some(certificate) = oracle.check(terms) {
                            return Ok(SatSearchOutcome::NoFixedPoint { certificate, stats });
                        }
                    }
                }
                if options.generalize {
                    encoding.block(&failure.obstruction)?;
                    obstructions.push(failure.obstruction);
                    stats.generalized_blockers += 1;
                } else {
                    encoding.block_exact(&candidate)?;
                    stats.exact_blockers += 1;
                }
            }
            stats.exhausted_sizes.push(nodes);
            if let Some(control) = control {
                control.record_diagram_stats(&stats);
            }
            eh.notify(
                "SAT: size proved impossible",
                nodes,
                options.max_nodes.unwrap_or(0),
            );
            if options.max_nodes == Some(nodes) {
                return Ok(SatSearchOutcome::Exhausted {
                    min_nodes: options.min_nodes,
                    max_nodes: nodes,
                    stats,
                });
            }
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| "SAT diagram size overflow".to_string())?;
        }
    }
}

#[cfg(test)]
mod tests;
