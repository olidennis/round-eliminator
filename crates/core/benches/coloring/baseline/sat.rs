//! LLM: Backend selection, formula construction, and generic SAT cardinality constraints.
//! LLM: Problem encodings depend on `Solve`, never on backend-specific types.

use rustsat::{
    encodings::{CollectClauses, am1, card},
    instances::{BasicVarManager, ManageVars},
    solvers::Solve,
    types::{Clause, Lit},
};

use crate::computation::{Cancellation, Cancelled};

/// LLM: Create the native CaDiCaL backend and attach a cooperative termination callback. Used
/// LLM: by SAT-based analyses through RustSAT's generic solver interfaces.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn backend(cancellation: &Cancellation) -> rustsat_cadical::CaDiCaL<'static, 'static> {
    // LLM: Prepare the local constants, types, or helper imports used by the following work.
    use rustsat::solvers::{ControlSignal, Terminate};

    // LLM: Create the target-specific backend before installing its shared cancellation
    // LLM: callback.
    let mut solver = rustsat_cadical::CaDiCaL::default();
    let cancellation = cancellation.clone();
    // LLM: Update the current working state before continuing this operation.
    solver.attach_terminator(move || {
        if cancellation.is_cancelled() {
            ControlSignal::Terminate
        } else {
            ControlSignal::Continue
        }
    });
    solver
}

/// LLM: Create the pure Rust BatSat backend and attach its stop callback. Used by SAT-based
/// LLM: analyses through RustSAT's generic solver interfaces.
#[cfg(target_arch = "wasm32")]
pub(crate) fn backend(cancellation: &Cancellation) -> rustsat_batsat::BasicSolver {
    // LLM: Create the target-specific backend before installing its shared cancellation
    // LLM: callback.
    let mut solver = rustsat_batsat::BasicSolver::default();
    let cancellation = cancellation.clone();
    solver
        .batsat_mut()
        .cb_mut()
        .set_stop(move || cancellation.is_cancelled());
    solver
}

/// LLM: A generic RustSAT solver, shared variable allocator, and cancellation flag. Encodings
/// LLM: allocate through this wrapper so auxiliary variables cannot collide and clause
/// LLM: generation remains interruptible.
pub(crate) struct Formula<S> {
    pub solver: S,
    variables: BasicVarManager,
    cancellation: Cancellation,
}

impl<S: Solve> Formula<S> {
    /// LLM: Create the generic solver wrapper and its shared auxiliary-variable allocator.
    /// LLM: Used before any solvability encoding is generated.
    pub fn new(solver: S, cancellation: Cancellation) -> Self {
        // LLM: Construct the stored representation from the prepared fields.
        Self {
            solver,
            variables: BasicVarManager::default(),
            cancellation,
        }
    }

    /// LLM: Allocate a fresh literal in the shared SAT variable space. Used by all constraint
    /// LLM: and symmetry encoders to prevent variable collisions.
    pub fn new_lit(&mut self) -> Lit {
        // LLM: Use the stored state for this operation without exposing mutable representation
        // LLM: details.
        self.variables.new_lit()
    }

    /// LLM: Check cancellation and send one clause to the backend. Used by generic encodings
    /// LLM: and counterexample refinement.
    pub fn add_clause(&mut self, literals: &[Lit]) -> anyhow::Result<()> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        self.check()?;
        self.solver.add_clause_ref(literals)
    }

    /// LLM: Require one and only one offered literal to hold. Used for deterministic per-port
    /// LLM: zero-round choices; an empty list is UNSAT.
    pub fn exactly_one(&mut self, literals: &[Lit]) -> anyhow::Result<()> {
        // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
        // LLM: violating combination.
        self.add_clause(literals)?;
        self.at_most(literals, 1)
    }

    /// LLM: Encode a capacity bound using RustSAT's ladder or totalizer and the shared
    /// LLM: variable manager. Used for assignment uniqueness and condensed-part capacities.
    pub fn at_most(&mut self, literals: &[Lit], capacity: usize) -> anyhow::Result<()> {
        // LLM: Prepare the local constants, types, or helper imports used by the following
        // LLM: work.
        use am1::Encode;
        use card::BoundUpper;

        // LLM: Observe cooperative interruption before continuing this unit of work.
        self.check()?;
        if capacity >= literals.len() {
            return Ok(());
        }
        // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
        // LLM: violating combination.
        if capacity == 0 {
            for &literal in literals {
                self.add_clause(&[!literal])?;
            }
            return Ok(());
        }
        // LLM: Stream library-generated clauses directly to the solver while saving
        // LLM: cancellation/backend failures.
        let mut clauses = EncodingClauses {
            solver: &mut self.solver,
            cancellation: &self.cancellation,
            count: 0,
            error: None,
        };
        // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
        // LLM: violating combination.
        let encoded = (|| -> anyhow::Result<()> {
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            if capacity == 1 {
                am1::Ladder::from(literals.to_vec()).encode(&mut clauses, &mut self.variables)?;
            } else {
                let mut encoder = card::Totalizer::from(literals.to_vec());
                encoder.encode_ub(capacity..=capacity, &mut clauses, &mut self.variables)?;
                for literal in encoder.enforce_ub(capacity)? {
                    clauses.add_clause(rustsat::clause![literal])?;
                }
            }
            // LLM: Return the result of this completed search or conversion to its caller.
            Ok(())
        })();
        // LLM: CollectClauses only exposes OutOfMemory. Restore the actual cause when
        // LLM: our collector stopped clause generation for cancellation or a solver error.

        match clauses.error {
            Some(error) => Err(error),
            None => encoded,
        }
    }

    /// LLM: Decide the requested coloring palette or maximum while retaining original inputs
    /// LLM: and mapping restrictions. Used by the shared API and direct callers; applicable
    /// LLM: cached proofs initialize the search rather than changing the promise.
    pub fn check(&self) -> Result<(), Cancelled> {
        // LLM: Observe cooperative interruption before continuing this unit of work.
        self.cancellation.check()
    }
}

/// LLM: A streaming adapter from RustSAT encoders to the underlying solver. It counts accepted
/// LLM: clauses and saves the actual cancellation/backend failure because the collector trait
/// LLM: exposes only an out-of-memory error channel.
struct EncodingClauses<'a, S> {
    solver: &'a mut S,
    cancellation: &'a Cancellation,
    count: usize,
    error: Option<anyhow::Error>,
}

impl<S: Solve> CollectClauses for EncodingClauses<'_, S> {
    /// LLM: Report the number of clauses accepted by this collector. Called by RustSAT
    /// LLM: encoding interfaces.
    fn n_clauses(&self) -> usize {
        // LLM: Read the stored data through the accessor, preserving private representation
        // LLM: invariants.
        self.count
    }

    /// LLM: Stream clauses with per-clause cancellation and preserve the original
    /// LLM: backend/error cause. Called by RustSAT cardinality encoders.
    fn extend_clauses<T>(&mut self, clauses: T) -> Result<(), rustsat::OutOfMemory>
    where
        T: IntoIterator<Item = Clause>,
    {
        // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
        // LLM: violating combination.
        for clause in clauses {
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            let added = self
                .cancellation
                .check()
                .map_err(anyhow::Error::from)
                .and_then(|()| self.solver.add_clause(clause));
            // LLM: Save the actual cause before returning the collector trait's limited abort
            // LLM: signal.
            if let Err(error) = added {
                self.error = Some(error);
                // LLM: This trait has no cancellation/error channel. The caller always
                // LLM: translates this abort signal back to the original stored error.

                return Err(rustsat::OutOfMemory::ExternalApi);
            }
            self.count += 1;
        }
        // LLM: Return the result of this completed search or conversion to its caller.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustsat::solvers::SolverResult;

    /// LLM: Regression test: verify that backend can be interrupted during search and repeated
    /// LLM: calls. Used by the test suite to protect this behavior while implementations
    /// LLM: change.
    #[test]
    fn backend_can_be_interrupted_during_search_and_repeated_calls() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let cancel_worker = cancellation.clone();
        let (started, ready) = std::sync::mpsc::channel();
        let (finished, result) = std::sync::mpsc::channel();
        // LLM: Search the current formula and distinguish SAT, UNSAT, and interruption.
        let worker = std::thread::spawn(move || {
            // LLM: Prepare `solver` for the following computation; it stores the solver for
            // LLM: this stage.
            let mut solver = backend(&cancel_worker);
            // LLM: Twenty-one pigeons in twenty holes gives a nontrivial search.

            let holes = 20;
            let pigeons = holes + 1;
            // LLM: Add the necessary SAT restriction; a refinement clause forbids the selected
            // LLM: violating combination.
            for pigeon in 0..pigeons {
                // LLM: Add the necessary SAT restriction; a refinement clause forbids the
                // LLM: selected violating combination.
                Solve::add_clause(
                    &mut solver,
                    (0..holes)
                        .map(|hole| Lit::positive(pigeon * holes + hole))
                        .collect(),
                )
                .unwrap();
            }
            // LLM: Visit the remaining alternatives for this stage, stopping when the required
            // LLM: search condition is settled.
            for hole in 0..holes {
                // LLM: Visit the remaining alternatives for this stage, stopping when the
                // LLM: required search condition is settled.
                for a in 0..pigeons {
                    // LLM: Visit the remaining alternatives for this stage, stopping when the
                    // LLM: required search condition is settled.
                    for b in a + 1..pigeons {
                        // LLM: Update the current working state before continuing this
                        // LLM: operation.
                        solver
                            .add_binary(
                                !Lit::positive(a * holes + hole),
                                !Lit::positive(b * holes + hole),
                            )
                            .unwrap();
                    }
                }
            }
            // LLM: Update the current working state before continuing this operation.
            started.send(()).unwrap();
            let first = solver.solve().unwrap();
            // LLM: The same cancellation flag must remain set on the next solve.

            let second = solver.solve().unwrap();
            finished.send((first, second)).unwrap();
        });
        // LLM: Update the current working state before continuing this operation.
        ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        cancellation.cancel();
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert_eq!(
            result
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            (SolverResult::Interrupted, SolverResult::Interrupted)
        );
        worker.join().unwrap();
    }

    /// LLM: Regression test: verify that cardinality encoding accepts exactly the assignments
    /// LLM: within the bound. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn cardinality_encoding_accepts_exactly_the_assignments_within_the_bound() {
        // LLM: Ask the incremental solver under the current activation gates; interruption is
        // LLM: not a negative proof.
        for size in 0..=6 {
            // LLM: Ask the incremental solver under the current activation gates; interruption
            // LLM: is not a negative proof.
            for capacity in 0..=size {
                // LLM: Share the stop flag with the work performed in this stage.
                let cancellation = crate::computation::Cancellation::default();
                let mut solver =
                    Formula::new(crate::algorithms::sat::backend(&cancellation), cancellation);
                let literals: Vec<_> = (0..size).map(|_| solver.new_lit()).collect();
                solver.at_most(&literals, capacity).unwrap();
                // LLM: Ask the incremental solver under the current activation gates;
                // LLM: interruption is not a negative proof.
                for mask in 0usize..(1 << size) {
                    // LLM: Enable exactly the requested palette and disable memberships
                    // LLM: belonging to retired colors.
                    let assumptions: Vec<_> = literals
                        .iter()
                        .enumerate()
                        .map(|(index, &lit)| if mask & (1 << index) != 0 { lit } else { !lit })
                        .collect();
                    // LLM: Check the expected result and invariants for this fixture,
                    // LLM: including the relevant edge case.
                    assert_eq!(
                        rustsat::solvers::SolveIncremental::solve_assumps(
                            &mut solver.solver,
                            &assumptions
                        )
                        .unwrap()
                            == rustsat::solvers::SolverResult::Sat,
                        mask.count_ones() as usize <= capacity
                    );
                }
            }
        }
    }

    /// LLM: Regression test: verify that cardinality encoders share variables and accept
    /// LLM: negative literals. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn cardinality_encoders_share_variables_and_accept_negative_literals() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let mut formula = Formula::new(backend(&cancellation), cancellation);
        let literals: Vec<_> = (0..7)
            .map(|index| {
                let literal = formula.new_lit();
                if index % 2 == 0 { literal } else { !literal }
            })
            .collect();
        // LLM: Bound assignment counts so a selected row's part capacities cannot be exceeded.
        formula.at_most(&literals[..3], 1).unwrap();
        formula.at_most(&literals[3..], 2).unwrap();
        let tail = formula.new_lit();
        assert!(tail.var() > literals[6].var());
        formula.add_clause(&[tail]).unwrap();
        // LLM: Ask the incremental solver under the current activation gates; interruption is
        // LLM: not a negative proof.
        for mask in 0u32..128 {
            // LLM: Enable exactly the requested palette and disable memberships belonging to
            // LLM: retired colors.
            let mut assumptions: Vec<_> = literals
                .iter()
                .enumerate()
                .map(|(index, &literal)| {
                    if mask & (1 << index) != 0 {
                        literal
                    } else {
                        !literal
                    }
                })
                .collect();
            // LLM: Retain the newly constructed choices in the result for this stage.
            assumptions.push(tail);
            let within_bounds = (mask & 7).count_ones() <= 1 && (mask >> 3).count_ones() <= 2;
            // LLM: Check the expected result and invariants for this fixture, including the
            // LLM: relevant edge case.
            assert_eq!(
                rustsat::solvers::SolveIncremental::solve_assumps(
                    &mut formula.solver,
                    &assumptions
                )
                .unwrap()
                    == SolverResult::Sat,
                within_bounds,
            );
        }
    }

    /// LLM: Regression test: verify that library clause collection stops and preserves
    /// LLM: cancellation. Used by the test suite to protect this behavior while
    /// LLM: implementations change.
    #[test]
    fn library_clause_collection_stops_and_preserves_cancellation() {
        // LLM: Share the stop flag with the work performed in this stage.
        let cancellation = Cancellation::default();
        let mut solver = backend(&cancellation);
        let mut collector = EncodingClauses {
            solver: &mut solver,
            cancellation: &cancellation,
            count: 0,
            error: None,
        };
        // LLM: Prepare `clauses` for the following computation; it stores the clauses for this
        // LLM: stage.
        let clauses = (0..10).map(|index| {
            if index == 3 {
                cancellation.cancel();
            }
            rustsat::clause![Lit::positive(index)]
        });
        assert!(collector.extend_clauses(clauses).is_err());
        assert_eq!(collector.count, 3);
        // LLM: Check the expected result and invariants for this fixture, including the
        // LLM: relevant edge case.
        assert!(collector.error.unwrap().is::<Cancelled>());

        // LLM: Prepare `formula` for the following computation; it stores the formula for this
        // LLM: stage.
        let mut formula = Formula::new(solver, cancellation);
        assert!(formula.at_most(&[], 0).unwrap_err().is::<Cancelled>());
    }
}
