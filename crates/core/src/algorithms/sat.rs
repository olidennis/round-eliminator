//! Backend selection, formula construction, and generic SAT cardinality constraints.
//! Problem encodings depend on `Solve`, never on backend-specific types.

use rustsat::{
    encodings::{am1, card, CollectClauses},
    instances::{BasicVarManager, ManageVars},
    solvers::Solve,
    types::{Clause, Lit},
};

use crate::computation::{Cancellation, Cancelled};

/// We use CaDiCaL in native mode
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn backend(cancellation: &Cancellation) -> rustsat_cadical::CaDiCaL<'static, 'static> {
    use rustsat::solvers::{ControlSignal, Terminate};

    let mut solver = rustsat_cadical::CaDiCaL::default();
    let cancellation = cancellation.clone();
    solver.attach_terminator(move || {
        if cancellation.is_cancelled() {
            ControlSignal::Terminate
        } else {
            ControlSignal::Continue
        }
    });
    solver
}

/// We use BatSat in wasm mode
#[cfg(target_arch = "wasm32")]
pub(crate) fn backend(cancellation: &Cancellation) -> rustsat_batsat::BasicSolver {
    let mut solver = rustsat_batsat::BasicSolver::default();
    let cancellation = cancellation.clone();
    solver
        .batsat_mut()
        .cb_mut()
        .set_stop(move || cancellation.is_cancelled());
    solver
}

/// Some convenience wrapper around rustsat
pub(crate) struct Formula<S> {
    pub solver: S,
    variables: BasicVarManager,
    cancellation: Cancellation,
}

impl<S: Solve> Formula<S> {
    pub fn new(solver: S, cancellation: Cancellation) -> Self {
        Self {
            solver,
            variables: BasicVarManager::default(),
            cancellation,
        }
    }

    pub fn new_lit(&mut self) -> Lit {
        self.variables.new_lit()
    }

    pub fn add_clause(&mut self, literals: &[Lit]) -> anyhow::Result<()> {
        self.check()?;
        self.solver.add_clause_ref(literals)
    }

    /// Require exactly one literal to be true; an empty list is unsatisfiable.
    pub fn exactly_one(&mut self, literals: &[Lit]) -> anyhow::Result<()> {
        self.add_clause(literals)?;
        self.at_most(literals, 1)
    }

    /// Require at most `capacity` literals to be true.
    /// RustSAT's ladder handles at-most-one; a totalizer handles larger bounds.
    pub fn at_most(&mut self, literals: &[Lit], capacity: usize) -> anyhow::Result<()> {
        use am1::Encode;
        use card::BoundUpper;

        self.check()?;
        if capacity >= literals.len() {
            return Ok(());
        }
        if capacity == 0 {
            for &literal in literals {
                self.add_clause(&[!literal])?;
            }
            return Ok(());
        }
        let mut clauses = EncodingClauses {
            solver: &mut self.solver,
            cancellation: &self.cancellation,
            count: 0,
            error: None,
        };
        let encoded = (|| -> anyhow::Result<()> {
            if capacity == 1 {
                am1::Ladder::from(literals.to_vec()).encode(&mut clauses, &mut self.variables)?;
            } else {
                let mut encoder = card::Totalizer::from(literals.to_vec());
                encoder.encode_ub(capacity..=capacity, &mut clauses, &mut self.variables)?;
                for literal in encoder.enforce_ub(capacity)? {
                    clauses.add_clause(rustsat::clause![literal])?;
                }
            }
            Ok(())
        })();
        // CollectClauses only exposes OutOfMemory. Restore the actual cause when
        // our collector stopped clause generation for cancellation or a solver error.
        match clauses.error {
            Some(error) => Err(error),
            None => encoded,
        }
    }

    pub fn check(&self) -> Result<(), Cancelled> {
        self.cancellation.check()
    }
}

/// Stream library encodings to the solver, checking cancellation at each clause.
struct EncodingClauses<'a, S> {
    solver: &'a mut S,
    cancellation: &'a Cancellation,
    count: usize,
    error: Option<anyhow::Error>,
}

impl<S: Solve> CollectClauses for EncodingClauses<'_, S> {
    fn n_clauses(&self) -> usize {
        self.count
    }

    fn extend_clauses<T>(&mut self, clauses: T) -> Result<(), rustsat::OutOfMemory>
    where
        T: IntoIterator<Item = Clause>,
    {
        for clause in clauses {
            let added = self
                .cancellation
                .check()
                .map_err(anyhow::Error::from)
                .and_then(|()| self.solver.add_clause(clause));
            if let Err(error) = added {
                self.error = Some(error);
                // This trait has no cancellation/error channel. The caller always
                // translates this abort signal back to the original stored error.
                return Err(rustsat::OutOfMemory::ExternalApi);
            }
            self.count += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustsat::solvers::SolverResult;

    #[test]
    fn backend_can_be_interrupted_during_search_and_repeated_calls() {
        let cancellation = Cancellation::default();
        let cancel_worker = cancellation.clone();
        let (started, ready) = std::sync::mpsc::channel();
        let (finished, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut solver = backend(&cancel_worker);
            // Twenty-one pigeons in twenty holes gives a nontrivial search.
            let holes = 20;
            let pigeons = holes + 1;
            for pigeon in 0..pigeons {
                Solve::add_clause(
                    &mut solver,
                    (0..holes)
                        .map(|hole| Lit::positive(pigeon * holes + hole))
                        .collect(),
                )
                .unwrap();
            }
            for hole in 0..holes {
                for a in 0..pigeons {
                    for b in a + 1..pigeons {
                        solver
                            .add_binary(
                                !Lit::positive(a * holes + hole),
                                !Lit::positive(b * holes + hole),
                            )
                            .unwrap();
                    }
                }
            }
            started.send(()).unwrap();
            let first = solver.solve().unwrap();
            // The same cancellation flag must remain set on the next solve.
            let second = solver.solve().unwrap();
            finished.send((first, second)).unwrap();
        });
        ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        cancellation.cancel();
        assert_eq!(
            result
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            (SolverResult::Interrupted, SolverResult::Interrupted)
        );
        worker.join().unwrap();
    }

    #[test]
    fn cardinality_encoding_accepts_exactly_the_assignments_within_the_bound() {
        for size in 0..=6 {
            for capacity in 0..=size {
                let cancellation = crate::computation::Cancellation::default();
                let mut solver =
                    Formula::new(crate::algorithms::sat::backend(&cancellation), cancellation);
                let literals: Vec<_> = (0..size).map(|_| solver.new_lit()).collect();
                solver.at_most(&literals, capacity).unwrap();
                for mask in 0usize..(1 << size) {
                    let assumptions: Vec<_> = literals
                        .iter()
                        .enumerate()
                        .map(|(index, &lit)| if mask & (1 << index) != 0 { lit } else { !lit })
                        .collect();
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

    #[test]
    fn cardinality_encoders_share_variables_and_accept_negative_literals() {
        let cancellation = Cancellation::default();
        let mut formula = Formula::new(backend(&cancellation), cancellation);
        let literals: Vec<_> = (0..7)
            .map(|index| {
                let literal = formula.new_lit();
                if index % 2 == 0 {
                    literal
                } else {
                    !literal
                }
            })
            .collect();
        formula.at_most(&literals[..3], 1).unwrap();
        formula.at_most(&literals[3..], 2).unwrap();
        let tail = formula.new_lit();
        assert!(tail.var() > literals[6].var());
        formula.add_clause(&[tail]).unwrap();
        for mask in 0u32..128 {
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
            assumptions.push(tail);
            let within_bounds = (mask & 7).count_ones() <= 1 && (mask >> 3).count_ones() <= 2;
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

    #[test]
    fn library_clause_collection_stops_and_preserves_cancellation() {
        let cancellation = Cancellation::default();
        let mut solver = backend(&cancellation);
        let mut collector = EncodingClauses {
            solver: &mut solver,
            cancellation: &cancellation,
            count: 0,
            error: None,
        };
        let clauses = (0..10).map(|index| {
            if index == 3 {
                cancellation.cancel();
            }
            rustsat::clause![Lit::positive(index)]
        });
        assert!(collector.extend_clauses(clauses).is_err());
        assert_eq!(collector.count, 3);
        assert!(collector.error.unwrap().is::<Cancelled>());

        let mut formula = Formula::new(solver, cancellation);
        assert!(formula.at_most(&[], 0).unwrap_err().is::<Cancelled>());
    }
}
