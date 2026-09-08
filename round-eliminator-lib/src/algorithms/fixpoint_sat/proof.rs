//! SAT synthesis of finite active derivation DAGs, independent of any diagram.
//!
//! C(x,y) denotes is_pred(mirror(x),y). Its ground facts come from the proof
//! oracle. Decomposing either argument gives OR at a join and AND at a meet;
//! the two applicable decompositions are ORed (exactly the existing recursion).
//! Every child precedes its parent, so these Boolean equations are acyclic.
//! No finite-lattice interpretation, guessed compatibility, or mirror nodes
//! are involved. A model is replayed as a legal active derivation and checked
//! again by the independent existing oracle before publishing a certificate.

use super::*;
use std::collections::BTreeMap;
use std::sync::mpsc::Receiver;

#[derive(Clone, Debug, Default)]
pub struct CertificateSearchOptions {
    /// Number of combination steps, not lattice nodes. None is unbounded.
    /// Shared, already-derived tuples can be used as additional leaves.
    pub max_steps: Option<usize>,
    /// Optional per-bound SAT conflict budget. Interruption is inconclusive.
    pub conflict_limit: Option<u32>,
}

#[derive(Debug)]
pub enum CertificateSearchOutcome {
    Found {
        certificate: String,
        steps: usize,
        shared_lines: usize,
    },
    /// No certificate in this bounded derivation grammar; NOT a good diagram.
    Exhausted {
        steps: usize,
    },
    Inconclusive {
        steps: usize,
    },
}

pub(super) fn validate(problem: &Problem, _: &CertificateSearchOptions) -> Result<(), String> {
    if !matches!(problem.active.degree, Degree::Finite(n) if n > 0)
        || problem.passive.degree != Degree::Finite(2)
    {
        return Err(
            "Certificate search requires finite positive active degree and passive degree 2".into(),
        );
    }
    Ok(())
}

struct Circuit<'a> {
    solver: Minisat,
    next_var: u32,
    truth: Lit,
    control: &'a SearchControl,
    gates: HashMap<Vec<Lit>, Lit>,
    // Only the standalone benchmark exporter records clauses. Normal Loop
    // keeps its existing memory use and incremental native solver interface.
    recorded: Option<Vec<Vec<Lit>>>,
}

impl<'a> Circuit<'a> {
    fn new(control: &'a SearchControl) -> Result<Self, String> {
        let truth = Lit::new(0, false);
        let mut solver = Minisat::default();
        solver
            .add_clause([truth].into_iter().collect())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            solver,
            next_var: 1,
            truth,
            control,
            gates: HashMap::new(),
            recorded: None,
        })
    }

    fn literal(&mut self) -> Result<Lit, String> {
        let v = self.next_var;
        self.next_var = v
            .checked_add(1)
            .ok_or("Proof SAT variable limit exceeded")?;
        Ok(Lit::new(v, false))
    }

    fn clause(&mut self, literals: impl IntoIterator<Item = Lit>) -> Result<(), String> {
        self.control.check()?;
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
        if let Some(recorded) = &mut self.recorded {
            recorded.push(literals.clone());
        }
        self.solver
            .add_clause(literals.into_iter().collect())
            .map_err(|e| e.to_string())
    }

    fn or(&mut self, mut terms: Vec<Lit>) -> Result<Lit, String> {
        if terms.contains(&self.truth) {
            return Ok(self.truth);
        }
        terms.retain(|l| *l != !self.truth);
        terms.sort_unstable();
        terms.dedup();
        if terms.iter().any(|l| terms.binary_search(&!*l).is_ok()) {
            return Ok(self.truth);
        }
        match terms.len() {
            0 => return Ok(!self.truth),
            1 => return Ok(terms[0]),
            _ => {}
        }
        if let Some(&lit) = self.gates.get(&terms) {
            return Ok(lit);
        }
        let result = self.literal()?;
        for &term in &terms {
            self.clause([!term, result])?;
        }
        self.clause(terms.iter().copied().chain([!result]))?;
        self.gates.insert(terms, result);
        Ok(result)
    }

    fn and(&mut self, terms: Vec<Lit>) -> Result<Lit, String> {
        Ok(!self.or(terms.into_iter().map(|l| !l).collect())?)
    }

    fn one_hot(&mut self, count: usize) -> Result<Vec<Lit>, String> {
        if count == 1 {
            return Ok(vec![self.truth]);
        }
        let row = (0..count)
            .map(|_| self.literal())
            .collect::<Result<Vec<_>, _>>()?;
        self.clause(row.iter().copied())?;
        // Sequential at-most-one, linear instead of quadratic in proof size.
        let mut previous = !self.truth;
        for &lit in &row {
            self.clause([!previous, !lit])?;
            previous = self.or(vec![previous, lit])?;
        }
        Ok(row)
    }

    // Selectors are collectively exactly one (possibly grouped when several
    // occurrences denote the same term). Grouping equal values shares gates.
    fn select(&mut self, choices: Vec<(Lit, Lit)>) -> Result<Lit, String> {
        let mut grouped = BTreeMap::<Lit, Vec<Lit>>::new();
        for (selector, value) in choices {
            grouped.entry(value).or_default().push(selector);
        }
        if grouped.len() == 1 {
            return Ok(*grouped.keys().next().unwrap());
        }
        let mut disjunction = Vec::new();
        for (value, selectors) in grouped {
            if value == !self.truth {
                continue;
            }
            let selector = self.or(selectors)?;
            disjunction.push(self.and(vec![selector, value])?);
        }
        self.or(disjunction)
    }
}

type Choices = Vec<(Lit, usize)>;

#[derive(Clone)]
enum Node {
    Atom(Label),
    Expr { union: bool, children: [Choices; 2] },
}

struct Parent {
    tuple: Vec<Lit>,
    permutation: Vec<Vec<Lit>>,
}

enum Source {
    Leaf(Vec<Term>),
    Combine([Parent; 2]),
}

struct Tuple {
    nodes: Vec<usize>,
    source: Source,
}

struct ProofEncoding<'a> {
    circuit: Circuit<'a>,
    nodes: Vec<Node>,
    compatibility: Vec<Vec<Lit>>,
    fixed: HashMap<Term, usize>,
    tuples: Vec<Tuple>,
    atomic: HashMap<(Label, Label), bool>,
    degree: usize,
}

impl<'a> ProofEncoding<'a> {
    fn new(
        original: &Problem,
        seeds: &[Vec<Term>],
        control: &'a SearchControl,
    ) -> Result<Self, String> {
        Self::new_recorded(original, seeds, control, false)
    }

    fn new_recorded(
        original: &Problem,
        seeds: &[Vec<Term>],
        control: &'a SearchControl,
        record: bool,
    ) -> Result<Self, String> {
        let mut oracle = NonexistenceOracle::new(original);
        let mut atomic = HashMap::new();
        for a in original.labels() {
            for b in original.labels() {
                atomic.insert((a, b), oracle.atomic_compatibility(a, b));
            }
        }
        // C is symmetric; verify the ground relation rather than silently
        // relying on a potentially asymmetric input/cache construction.
        if atomic
            .iter()
            .any(|(&(a, b), &value)| atomic[&(b, a)] != value)
        {
            return Err("Asymmetric ground compatibility in certificate oracle".into());
        }
        let mut result = Self {
            circuit: Circuit::new(control)?,
            nodes: Vec::new(),
            compatibility: Vec::new(),
            fixed: HashMap::new(),
            tuples: Vec::new(),
            atomic,
            degree: original.active.finite_degree(),
        };
        if record {
            result.circuit.recorded = Some(vec![vec![result.circuit.truth]]);
        }
        for terms in seeds {
            let nodes = terms
                .iter()
                .map(|t| result.fixed_term(t))
                .collect::<Result<Vec<_>, _>>()?;
            result.tuples.push(Tuple {
                nodes,
                source: Source::Leaf(terms.clone()),
            });
        }
        Ok(result)
    }

    fn compatible(&self, a: usize, b: usize) -> Lit {
        self.compatibility[a.max(b)][a.min(b)]
    }

    fn decomposition(&mut self, a: usize, b: usize) -> Result<Lit, String> {
        match self.nodes[a].clone() {
            Node::Atom(_) => Ok(!self.circuit.truth),
            Node::Expr { union, children } => {
                let mut sides = Vec::new();
                for child in children {
                    let choices = child
                        .iter()
                        .map(|&(s, c)| (s, self.compatible(c, b)))
                        .collect();
                    sides.push(self.circuit.select(choices)?);
                }
                if union {
                    self.circuit.or(sides)
                } else {
                    self.circuit.and(sides)
                }
            }
        }
    }

    fn node(&mut self, node: Node) -> Result<usize, String> {
        self.circuit.control.check()?;
        let b = self.nodes.len();
        self.nodes.push(node);
        self.compatibility.push(Vec::with_capacity(b + 1));
        for a in 0..=b {
            let value = match (&self.nodes[a], &self.nodes[b]) {
                (Node::Atom(x), Node::Atom(y)) => {
                    if self.atomic[&(*x, *y)] {
                        self.circuit.truth
                    } else {
                        !self.circuit.truth
                    }
                }
                _ => {
                    let left = self.decomposition(a, b)?;
                    let right = self.decomposition(b, a)?;
                    self.circuit.or(vec![left, right])?
                }
            };
            self.compatibility[b].push(value);
        }
        Ok(b)
    }

    fn fixed_term(&mut self, term: &Term) -> Result<usize, String> {
        if let Some(&id) = self.fixed.get(term) {
            return Ok(id);
        }
        let node = match term {
            Term::Terminal(label) => Node::Atom(*label),
            Term::Expr(a, b, op) => {
                let a = self.fixed_term(a)?;
                let b = self.fixed_term(b)?;
                Node::Expr {
                    union: *op == Operation::Union,
                    children: [vec![(self.circuit.truth, a)], vec![(self.circuit.truth, b)]],
                }
            }
        };
        let id = self.node(node)?;
        self.fixed.insert(term.clone(), id);
        Ok(id)
    }

    fn parent(&mut self) -> Result<(Parent, Vec<Choices>), String> {
        let tuple = self.circuit.one_hot(self.tuples.len())?;
        let permutation = (0..self.degree)
            .map(|_| self.circuit.one_hot(self.degree))
            .collect::<Result<Vec<_>, _>>()?;
        // Rows are one-hot; column uniqueness makes an occurrence-preserving
        // permutation, even when several occurrences have identical labels.
        for column in 0..self.degree {
            for a in 0..self.degree {
                for b in a + 1..self.degree {
                    self.circuit
                        .clause([!permutation[a][column], !permutation[b][column]])?;
                }
            }
        }
        let mut children = Vec::new();
        for row in &permutation {
            let mut choices = BTreeMap::<usize, Vec<Lit>>::new();
            for (t, &selected) in self.tuples.iter().zip(&tuple) {
                for (&id, &position) in t.nodes.iter().zip(row) {
                    choices
                        .entry(id)
                        .or_default()
                        .push(self.circuit.and(vec![selected, position])?);
                }
            }
            children.push(
                choices
                    .into_iter()
                    .map(|(id, lits)| Ok((self.circuit.or(lits)?, id)))
                    .collect::<Result<Vec<_>, String>>()?,
            );
        }
        Ok((Parent { tuple, permutation }, children))
    }

    fn step(&mut self) -> Result<Lit, String> {
        let (left, left_children) = self.parent()?;
        let (right, right_children) = self.parent()?;
        // Union/intersection are commutative; keep one parent ordering.
        for a in 0..left.tuple.len() {
            for b in 0..a {
                self.circuit.clause([!left.tuple[a], !right.tuple[b]])?;
            }
        }
        let mut nodes = Vec::new();
        for (coordinate, (left, right)) in left_children.into_iter().zip(right_children).enumerate()
        {
            // Fixing the pivot to coordinate zero loses nothing: both input
            // permutations are free, as are permutations at every later use.
            nodes.push(self.node(Node::Expr {
                union: coordinate == 0,
                children: [left, right],
            })?);
        }
        let mut pairs = Vec::new();
        for a in 0..self.degree {
            for b in 0..=a {
                pairs.push(self.compatible(nodes[a], nodes[b]));
            }
        }
        self.tuples.push(Tuple {
            nodes,
            source: Source::Combine([left, right]),
        });
        self.circuit.and(pairs)
    }

    fn replay(&self, assignment: &Assignment, root: usize) -> Result<Vec<Term>, String> {
        fn selected(row: &[Lit], assignment: &Assignment) -> Result<usize, String> {
            let choices: Vec<_> = row
                .iter()
                .enumerate()
                .filter(|(_, l)| assignment.lit_value(**l) == TernaryVal::True)
                .map(|(i, _)| i)
                .collect();
            if choices.len() != 1 {
                return Err("Invalid proof SAT choice".into());
            }
            Ok(choices[0])
        }
        let mut values: Vec<Vec<Term>> = Vec::new();
        for tuple in self.tuples.iter().take(root + 1) {
            self.circuit.control.check()?;
            let terms = match &tuple.source {
                Source::Leaf(terms) => terms.clone(),
                Source::Combine(parents) => {
                    let mut children = Vec::new();
                    for parent in parents {
                        let source = &values[selected(&parent.tuple, assignment)?];
                        let perm = parent
                            .permutation
                            .iter()
                            .map(|r| selected(r, assignment))
                            .collect::<Result<Vec<_>, _>>()?;
                        if perm.iter().copied().collect::<HashSet<_>>().len() != self.degree {
                            return Err("Invalid proof occurrence permutation".into());
                        }
                        children.push(
                            perm.into_iter()
                                .map(|i| source[i].clone())
                                .collect::<Vec<_>>(),
                        );
                    }
                    children[0]
                        .iter()
                        .zip(&children[1])
                        .enumerate()
                        .map(|(i, (a, b))| {
                            if a == b {
                                a.clone()
                            } else {
                                Term::Expr(
                                    Box::new(a.clone()),
                                    Box::new(b.clone()),
                                    if i == 0 {
                                        Operation::Union
                                    } else {
                                        Operation::Intersection
                                    },
                                )
                            }
                        })
                        .collect()
                }
            };
            values.push(terms);
        }
        values.pop().ok_or_else(|| "Missing proof root".into())
    }
}

fn input_terms(original: &Problem) -> Vec<Vec<Term>> {
    let mut inputs: Vec<_> = original
        .active
        .all_choices(true)
        .iter()
        .map(|l| {
            expanded(l)
                .into_iter()
                .map(Term::Terminal)
                .collect::<Vec<_>>()
        })
        .collect();
    for terms in &mut inputs {
        terms.sort();
    }
    inputs.sort();
    inputs.dedup();
    inputs
}

pub(super) fn run(
    original: &Problem,
    options: &CertificateSearchOptions,
    eh: &mut EventHandler,
    control: &SearchControl,
    hints: Option<&Receiver<Vec<Term>>>,
) -> Result<CertificateSearchOutcome, String> {
    control.check()?;
    validate(original, options)?;
    let mut oracle = NonexistenceOracle::new(original);
    let mut seeds = input_terms(original);
    let inputs = seeds.len();
    for terms in &seeds {
        control.check()?;
        if let Some(certificate) = oracle.check(terms) {
            return Ok(CertificateSearchOutcome::Found {
                certificate,
                steps: 0,
                shared_lines: 0,
            });
        }
    }
    if seeds.is_empty() || options.max_steps == Some(0) {
        return Ok(CertificateSearchOutcome::Exhausted { steps: 0 });
    }
    let mut encoding = ProofEncoding::new(original, &seeds, control)?;
    let mut goals = Vec::new();
    let mut exhausted = Vec::new();
    let mut budgets: Vec<i64> = Vec::new();
    let mut next = 0;
    loop {
        control.check()?;
        // Hints are an optional accelerator, never the enumerated grammar's
        // only source of new expressions. Bound their import cost and memory.
        // Rebuilding at the SAME bound makes every accepted hint selectable
        // by every proof step; keeping it only for later steps would miss this.
        let mut changed = false;
        if let Some(hints) = hints {
            for mut terms in hints.try_iter().take(64) {
                control.check()?;
                terms.sort();
                if seeds.contains(&terms) {
                    continue;
                }
                if let Some(certificate) = oracle.check(&terms) {
                    return Ok(CertificateSearchOutcome::Found {
                        certificate,
                        steps: 0,
                        shared_lines: seeds.len() - inputs + 1,
                    });
                }
                let mut subterms = HashSet::new();
                let mut pending: Vec<_> = seeds.iter().flatten().chain(&terms).collect();
                while let Some(t) = pending.pop() {
                    if subterms.insert(t) {
                        if let Term::Expr(a, b, _) = t {
                            pending.push(a);
                            pending.push(b);
                        }
                    }
                }
                if seeds.len() - inputs < 32 && subterms.len() <= 256 {
                    seeds.push(terms);
                    changed = true;
                }
            }
        }
        if changed {
            eh.notify(
                "Proof: incorporating shared game derivations",
                seeds.len() - inputs,
                0,
            );
            encoding = ProofEncoding::new(original, &seeds, control)?;
            for goal in &mut goals {
                *goal = encoding.step()?;
            }
            exhausted.fill(false);
            budgets.fill(10_000);
            next = 0;
        }
        // Fair, deterministic rounds over unresolved bounds. A difficult
        // short proof bound must not prevent us trying longer proofs. Keep
        // every unfinished goal and its learned clauses, and revisit it in
        // every later round; an interrupted bound is NEVER called exhausted.
        if next == goals.len() {
            if options.max_steps == Some(goals.len()) {
                if exhausted.iter().all(|done| *done) {
                    return Ok(CertificateSearchOutcome::Exhausted { steps: goals.len() });
                }
            } else {
                eh.notify(
                    "Proof: encoding derivation steps",
                    goals.len() + 1,
                    options.max_steps.unwrap_or(0),
                );
                goals.push(encoding.step()?);
                exhausted.push(false);
                budgets.push(10_000);
                eh.notify(
                    "Proof: extending derivation bound",
                    goals.len(),
                    options.max_steps.unwrap_or(0),
                );
            }
            next = 0;
        }
        while next < goals.len() && exhausted[next] {
            next += 1;
        }
        if next == goals.len() {
            continue;
        }
        let index = next;
        let steps = index + 1;
        next += 1;
        eh.notify(
            "Proof: searching derivation steps",
            steps,
            options.max_steps.unwrap_or(0),
        );
        // The SAME incremental solver supports every root through assumptions.
        // Additional steps always admit an extension of earlier derivations.
        encoding.circuit.solver.set_limit(Limit::Conflicts(
            options.conflict_limit.map_or(budgets[index], i64::from),
        ));
        match control.solve(1, &mut encoding.circuit.solver, Some(&[goals[index]]))? {
            SolverResult::Interrupted if options.conflict_limit.is_some() => {
                return Ok(CertificateSearchOutcome::Inconclusive { steps })
            }
            SolverResult::Interrupted => {
                budgets[index] = budgets[index].saturating_mul(2);
                eh.notify(
                    "Proof: unfinished bound retained for later rounds",
                    steps,
                    0,
                );
            }
            SolverResult::Unsat => {
                eh.notify(
                    "Proof: derivation bound exhausted",
                    steps,
                    options.max_steps.unwrap_or(0),
                );
                exhausted[index] = true;
            }
            SolverResult::Sat => {
                let assignment = encoding
                    .circuit
                    .solver
                    .full_solution()
                    .map_err(|e| e.to_string())?;
                let terms = encoding.replay(&assignment, seeds.len() + index)?;
                let certificate = oracle
                    .check(&terms)
                    .ok_or("Proof SAT encoding disagrees with the nonexistence oracle")?;
                eh.notify("Proof: universal certificate verified", steps, 0);
                return Ok(CertificateSearchOutcome::Found {
                    certificate,
                    steps,
                    shared_lines: seeds.len() - inputs,
                });
            }
        }
    }
}

impl Problem {
    /// Standalone deterministic proof search, useful for bounded experiments.
    /// Native GUI Loop instead runs this alongside finite-diagram synthesis.
    pub fn fixpoint_certificate(
        &self,
        options: &CertificateSearchOptions,
        eh: &mut EventHandler,
    ) -> Result<CertificateSearchOutcome, String> {
        validate(self, options)?;
        let mut original = self.clone();
        if original.diagram_indirect.is_none() {
            original.compute_diagram(eh);
        }
        run(&original, options, eh, &SearchControl::default(), None)
    }
}

#[cfg(test)]
mod tests;

pub mod instance;

pub(super) mod guided;
