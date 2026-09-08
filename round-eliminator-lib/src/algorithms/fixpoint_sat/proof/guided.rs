//! Small SAT neighborhoods of concrete, independently replayed game fragments.
//! This is an accelerator, not a replacement for the unbounded proof grammar.
//! In particular, local UNSAT never establishes existence of a good diagram.

use super::*;
use std::collections::VecDeque;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::Duration;

// Bounds are on this optional accelerator only. Oversized fragments still
// participate in the diagram oracle and the independent general proof search.
const MAX_FRAGMENTS: usize = 256;
const MAX_TREE_NODES: usize = 4096;
const BATCH_SIZE: usize = 12;
const MAX_FIXED_NODES: usize = 512;
const LOCAL_CONFLICTS: i64 = 2_000;
const SAVED_VARIABLES: usize = 300_000;
const MAX_LOCAL_STEPS: usize = 8;

#[derive(Clone, Debug)]
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
        Ok(values.into_iter().flatten().collect())
    }
}

struct Bank {
    tuples: Vec<Vec<Term>>,
    known: HashSet<Vec<Term>>,
    pending: VecDeque<Vec<usize>>,
    scheduled: HashSet<Vec<usize>>,
    inputs: usize,
}

impl Bank {
    fn new(inputs: Vec<Vec<Term>>) -> Self {
        let mut result = Self {
            inputs: inputs.len(),
            known: inputs.iter().cloned().collect(),
            tuples: inputs,
            pending: VecDeque::new(),
            scheduled: HashSet::new(),
        };
        result.schedule(0);
        result
    }

    fn enqueue(&mut self, ids: impl IntoIterator<Item = usize>) {
        let mut batch = Vec::new();
        let mut fixed = HashSet::new();
        for id in ids {
            let mut added = HashSet::new();
            let mut pending: Vec<_> = self.tuples[id].iter().collect();
            while let Some(t) = pending.pop() {
                if !fixed.contains(t) && added.insert(t.clone()) {
                    if let Term::Expr(a, b, _) = t {
                        pending.extend([a.as_ref(), b.as_ref()]);
                    }
                }
            }
            if fixed.len() + added.len() <= MAX_FIXED_NODES {
                fixed.extend(added);
                batch.push(id);
            }
        }
        batch.sort_unstable();
        batch.dedup();
        if !batch.is_empty() && self.scheduled.insert(batch.clone()) {
            // A pending smaller neighborhood is covered by its superset.
            self.pending
                .retain(|old| !old.iter().all(|id| batch.binary_search(id).is_ok()));
            self.pending.push_back(batch);
        }
    }

    fn schedule(&mut self, first_new: usize) {
        let count = self.tuples.len();
        if count <= BATCH_SIZE {
            self.enqueue(0..count);
            return;
        }
        // Revisit old fragments with new ones, not merely the first N hints.
        // Every pair of blocks has a neighborhood, including distant batches.
        let half = BATCH_SIZE / 2;
        for right in (first_new / half * half..count).step_by(half) {
            for left in (0..right + 1).step_by(half).rev() {
                let ids: Vec<_> = (right..(right + half).min(count))
                    .chain(left..(left + half).min(count))
                    .collect();
                self.enqueue(ids);
            }
        }
    }

    fn import(
        &mut self,
        derivation: Derivation,
        oracle: &mut NonexistenceOracle,
        control: &SearchControl,
        eh: &mut EventHandler,
    ) -> Result<Option<CertificateSearchOutcome>, String> {
        let first_new = self.tuples.len();
        let degree = self.tuples.first().map_or(0, Vec::len);
        let values = derivation.replay(&self.tuples[..self.inputs], degree, control)?;
        if values.len() < derivation.steps.len() {
            eh.notify(
                "Proof: guided oversized fragments skipped",
                derivation.steps.len() - values.len(),
                MAX_TREE_NODES,
            );
        }
        let mut skipped = 0;
        for mut terms in values {
            control.check()?;
            terms.sort();
            if self.known.contains(&terms) {
                continue;
            }
            if let Some(certificate) = oracle.check(&terms) {
                return Ok(Some(CertificateSearchOutcome::Found {
                    certificate,
                    steps: 0,
                    shared_lines: 1,
                }));
            }
            if self.tuples.len() - self.inputs >= MAX_FRAGMENTS {
                skipped += 1;
                continue;
            }
            self.known.insert(terms.clone());
            self.tuples.push(terms);
        }
        if self.tuples.len() != first_new {
            eh.notify(
                "Proof: guided replayed game fragments",
                self.tuples.len() - self.inputs,
                MAX_FRAGMENTS,
            );
            self.schedule(first_new);
        }
        if skipped > 0 {
            eh.notify(
                "Proof: guided fragment memory limit (general search continues)",
                skipped,
                0,
            );
        }
        Ok(None)
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
        Ok(Self {
            encoding: ProofEncoding::new(original, &seeds, control)?,
            leaves: seeds.len(),
            shared: ids.iter().filter(|&&i| i >= bank.inputs).count(),
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
            2,
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

struct Retry<'a> {
    ids: Vec<usize>,
    saved: Option<Neighborhood<'a>>,
    conflicts: i64,
    steps: usize,
}

pub(crate) fn run(
    original: &Problem,
    options: &CertificateSearchOptions,
    eh: &mut EventHandler,
    control: &SearchControl,
    hints: &Receiver<Derivation>,
) -> Result<CertificateSearchOutcome, String> {
    control.check()?;
    let max_steps = options
        .max_steps
        .unwrap_or(MAX_LOCAL_STEPS)
        .min(MAX_LOCAL_STEPS);
    let mut oracle = NonexistenceOracle::new(original);
    let mut bank = Bank::new(input_terms(original));
    if max_steps == 0 || bank.tuples.is_empty() {
        return Ok(CertificateSearchOutcome::Inconclusive { steps: 0 });
    }
    let mut closed = false;
    let mut retries: VecDeque<Retry> = VecDeque::new();
    let mut saved_variables = 0;
    let mut prefer_new = true;
    loop {
        control.check()?;
        // Drain at checkpoints, including between successive local SAT calls.
        // The diagram worker sends compact DAGs and never waits for this worker.
        for _ in 0..32 {
            match hints.try_recv() {
                Ok(hint) => {
                    if let Some(found) = bank.import(hint, &mut oracle, control, eh)? {
                        return Ok(found);
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    closed = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        // Alternate fresh batches and unfinished batches: neither a stream of
        // new witnesses nor one hard local instance may starve the other.
        let work = if !bank.pending.is_empty() && (prefer_new || retries.is_empty()) {
            prefer_new = false;
            Some(Retry {
                ids: bank.pending.pop_front().unwrap(),
                saved: None,
                conflicts: LOCAL_CONFLICTS,
                steps: max_steps.min(3),
            })
        } else {
            prefer_new = true;
            retries.pop_front()
        };
        let Some(mut work) = work else {
            if closed {
                // Even UNSAT for every batch is only a local result; the
                // general proof worker retains the unrestricted grammar.
                return Ok(CertificateSearchOutcome::Inconclusive { steps: max_steps });
            } else {
                match hints.recv_timeout(Duration::from_millis(100)) {
                    Ok(hint) => {
                        if let Some(found) = bank.import(hint, &mut oracle, control, eh)? {
                            return Ok(found);
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => closed = true,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                continue;
            }
        };
        let mut job = if let Some(saved) = work.saved.take() {
            saved_variables -= saved.encoding.circuit.next_var as usize;
            saved
        } else {
            Neighborhood::new(original, &bank, &work.ids, control)?
        };
        let local_options = CertificateSearchOptions {
            conflict_limit: Some(options.conflict_limit.unwrap_or(work.conflicts as u32)),
            ..options.clone()
        };
        // Leave deep original-only synthesis to the independent general
        // worker; widening here is for bridges involving actual fragments.
        let neighborhood_limit = if job.shared == 0 {
            max_steps.min(3)
        } else {
            max_steps
        };
        for index in 0..work.steps {
            if let Some(found) = job.search(index, &mut oracle, &local_options, eh)? {
                return Ok(found);
            }
        }
        let unfinished = job.exhausted.iter().any(|&done| !done);
        if work.steps < neighborhood_limit || (unfinished && options.conflict_limit.is_none()) {
            // Widen the local bridge even if an earlier bound is difficult.
            // Short-bound UNSAT is not a reason to wait indefinitely for the
            // diagram worker to discover a deeper fragment.
            work.steps = (work.steps + 1).min(neighborhood_limit);
            if unfinished {
                work.conflicts = work.conflicts.saturating_mul(2).min(u32::MAX as i64);
            }
            let variables = job.encoding.circuit.next_var as usize;
            if saved_variables + variables <= SAVED_VARIABLES {
                // Keep learned clauses whenever the cache budget permits.
                saved_variables += variables;
                work.saved = Some(job);
            }
            // A cold retry is still scheduled, with a larger budget. Memory
            // pressure must not turn an unfinished neighborhood into UNSAT.
            retries.push_back(work);
        }
    }
}

#[cfg(test)]
mod tests;
