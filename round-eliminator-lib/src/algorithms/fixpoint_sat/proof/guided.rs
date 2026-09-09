//! Small SAT neighborhoods of concrete, independently replayed game fragments.
//! This is an accelerator, not a replacement for the unbounded proof grammar.
//! In particular, local UNSAT never establishes existence of a good diagram.

use super::*;
use std::collections::VecDeque;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

mod bank;
mod feedback;
mod pool;
use bank::Bank;
pub(crate) use pool::settings as pool_settings;

// Bounds are on this optional accelerator only. Oversized fragments still
// participate in the diagram oracle and the independent general proof search.
const MAX_FRAGMENTS: usize = 256;
const MAX_TREE_NODES: usize = 4096;
const BATCH_SIZE: usize = 12;
const MAX_FIXED_NODES: usize = 512;
const LOCAL_CONFLICTS: i64 = 2_000;
const MAX_LOCAL_STEPS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Step {
    Input(Vec<Label>),
    Combine {
        parents: [usize; 2],
        permutations: [Vec<usize>; 2],
        pivot: usize,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Derivation {
    pub steps: Vec<Step>,
}

// Only universal lattice identities are used here, never equality in one
// candidate diagram. Do not sort coordinates until AFTER replaying the DAG.
fn combine(a: Term, b: Term, op: Operation) -> Term {
    if a == b {
        a
    } else if a < b {
        Term::Expr(Box::new(a), Box::new(b), op)
    } else {
        Term::Expr(Box::new(b), Box::new(a), op)
    }
}

fn tree_size(term: &Term) -> usize {
    let mut pending = vec![term];
    let mut size = 0;
    while let Some(term) = pending.pop() {
        size += 1;
        if let Term::Expr(a, b, _) = term {
            pending.extend([a.as_ref(), b.as_ref()]);
        }
    }
    size
}

impl Derivation {
    pub(crate) fn replay(
        &self,
        inputs: &[Vec<Term>],
        degree: usize,
        control: &SearchControl,
    ) -> Result<Vec<Vec<Term>>, String> {
        Ok(self
            .replay_indexed(inputs, degree, control)?
            .into_iter()
            .flatten()
            .collect())
    }

    fn replay_indexed(
        &self,
        inputs: &[Vec<Term>],
        degree: usize,
        control: &SearchControl,
    ) -> Result<Vec<Option<Vec<Term>>>, String> {
        let mut values: Vec<Option<Vec<Term>>> = Vec::new();
        let mut sizes: Vec<usize> = Vec::new();
        for step in &self.steps {
            control.check()?;
            let terms = match step {
                Step::Input(labels) => {
                    let terms: Vec<_> = labels.iter().copied().map(Term::Terminal).collect();
                    let mut sorted = terms.clone();
                    sorted.sort();
                    if terms.len() != degree || !inputs.contains(&sorted) {
                        return Err(
                            "Guided game derivation has an invalid input configuration".into()
                        );
                    }
                    Some(terms)
                }
                Step::Combine {
                    parents,
                    permutations,
                    pivot,
                } => {
                    if *pivot >= degree || parents.iter().any(|&p| p >= values.len()) {
                        return Err("Guided game derivation has an invalid or cyclic parent".into());
                    }
                    for permutation in permutations {
                        let mut sorted = permutation.clone();
                        sorted.sort_unstable();
                        if sorted != (0..degree).collect::<Vec<_>>() {
                            return Err(
                                "Guided game derivation does not preserve occurrences".into()
                            );
                        }
                    }
                    if sizes[parents[0]] + sizes[parents[1]] + degree > MAX_TREE_NODES {
                        None
                    } else {
                        let left = values[parents[0]].as_ref().unwrap();
                        let right = values[parents[1]].as_ref().unwrap();
                        Some(
                            (0..degree)
                                .map(|i| {
                                    combine(
                                        left[permutations[0][i]].clone(),
                                        right[permutations[1][i]].clone(),
                                        if i == *pivot {
                                            Operation::Union
                                        } else {
                                            Operation::Intersection
                                        },
                                    )
                                })
                                .collect(),
                        )
                    }
                }
            };
            sizes.push(
                terms
                    .as_ref()
                    .map_or(MAX_TREE_NODES + 1, |t| t.iter().map(tree_size).sum()),
            );
            values.push(terms);
        }
        Ok(values)
    }
}

struct Neighborhood<'a> {
    encoding: ProofEncoding<'a>,
    leaves: usize,
    shared: usize,
    goals: Vec<Lit>,
    exhausted: Vec<bool>,
}

impl<'a> Neighborhood<'a> {
    fn new(
        original: &Problem,
        bank: &Bank,
        ids: &[usize],
        control: &'a SearchControl,
    ) -> Result<Self, String> {
        let seeds: Vec<_> = ids.iter().map(|&i| bank.tuples[i].clone()).collect();
        Self::from_seeds(
            original,
            &seeds,
            ids.iter().filter(|&&i| i >= bank.inputs).count(),
            control,
        )
    }

    fn from_seeds(
        original: &Problem,
        seeds: &[Vec<Term>],
        shared: usize,
        control: &'a SearchControl,
    ) -> Result<Self, String> {
        Ok(Self {
            encoding: ProofEncoding::new(original, &seeds, control)?,
            leaves: seeds.len(),
            shared,
            goals: Vec::new(),
            exhausted: Vec::new(),
        })
    }

    fn search(
        &mut self,
        index: usize,
        oracle: &mut NonexistenceOracle,
        options: &CertificateSearchOptions,
        eh: &mut EventHandler,
    ) -> Result<Option<CertificateSearchOutcome>, String> {
        if self.exhausted.get(index) == Some(&true) {
            return Ok(None);
        }
        let steps = index + 1;
        let total = options
            .max_steps
            .unwrap_or(MAX_LOCAL_STEPS)
            .min(MAX_LOCAL_STEPS);
        if index == self.goals.len() {
            eh.notify(
                format!(
                    "Proof: guided encoding {} fragments + {steps} new steps",
                    self.leaves
                ),
                steps,
                total,
            );
            self.goals.push(self.encoding.step()?);
            self.exhausted.push(false);
            // Whole output-coordinate permutations are redundant. Keep the pivot
            // at zero and order the remaining left-parent occurrence columns.
            let Source::Combine(parents) = &self.encoding.tuples.last().unwrap().source else {
                unreachable!()
            };
            let permutation = parents[0].permutation.clone();
            for row in 1..self.encoding.degree.saturating_sub(1) {
                for a in 0..self.encoding.degree {
                    for b in 0..a {
                        self.encoding
                            .circuit
                            .clause([!permutation[row][a], !permutation[row + 1][b]])?;
                    }
                }
            }
        }
        let goal = self.goals[index];
        eh.notify(
            format!(
                "Proof: guided SAT ({} variables)",
                self.encoding.circuit.next_var
            ),
            steps,
            total,
        );
        self.encoding.circuit.solver.set_limit(Limit::Conflicts(
            options.conflict_limit.map_or(LOCAL_CONFLICTS, i64::from),
        ));
        match self.encoding.circuit.control.solve(
            self.encoding.circuit.worker,
            &mut self.encoding.circuit.solver,
            Some(&[goal]),
        )? {
            SolverResult::Sat => {
                let assignment = self
                    .encoding
                    .circuit
                    .solver
                    .full_solution()
                    .map_err(|e| e.to_string())?;
                let terms = self.encoding.replay(&assignment, self.leaves + index)?;
                let certificate = oracle
                    .check(&terms)
                    .ok_or("Guided SAT disagrees with the nonexistence oracle")?;
                eh.notify(
                    "Proof: guided universal certificate verified",
                    steps,
                    self.shared,
                );
                Ok(Some(CertificateSearchOutcome::Found {
                    certificate,
                    steps,
                    shared_lines: self.shared,
                }))
            }
            SolverResult::Unsat => {
                self.exhausted[index] = true;
                eh.notify(
                    "Proof: guided neighborhood exhausted (not a global conclusion)",
                    steps,
                    total,
                );
                Ok(None)
            }
            SolverResult::Interrupted => {
                eh.notify(
                    "Proof: guided neighborhood budget reached (inconclusive)",
                    steps,
                    total,
                );
                Ok(None)
            }
        }
    }
}

pub(crate) fn run(
    original: &Problem,
    options: &CertificateSearchOptions,
    eh: &mut EventHandler,
    control: &SearchControl,
    hints: &Receiver<Derivation>,
) -> Result<CertificateSearchOutcome, String> {
    run_inner(original, options, eh, control, hints, false, None)
}

pub(crate) fn run_with_default_seed(
    original: &Problem,
    options: &CertificateSearchOptions,
    eh: &mut EventHandler,
    control: &SearchControl,
    hints: &Receiver<Derivation>,
    shared: &std::sync::mpsc::SyncSender<Vec<Term>>,
) -> Result<CertificateSearchOutcome, String> {
    run_inner(original, options, eh, control, hints, true, Some(shared))
}

fn run_inner(
    original: &Problem,
    options: &CertificateSearchOptions,
    eh: &mut EventHandler,
    control: &SearchControl,
    hints: &Receiver<Derivation>,
    default_seed: bool,
    shared: Option<&std::sync::mpsc::SyncSender<Vec<Term>>>,
) -> Result<CertificateSearchOutcome, String> {
    pool::run(original, options, eh, control, hints, default_seed, shared)
}

#[cfg(test)]
mod tests;
