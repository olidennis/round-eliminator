//! Backend selection, formula construction, and generic SAT cardinality constraints.
//! Problem encodings depend on `Solve`, never on backend-specific types.

use rustsat::{solvers::Solve, types::Lit};

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
    next_variable: u32,
    cancellation: Cancellation,
}

impl<S: Solve> Formula<S> {
    pub fn new(solver: S, cancellation: Cancellation) -> Self {
        Self {
            solver,
            next_variable: 0,
            cancellation,
        }
    }

    pub fn new_lit(&mut self) -> Lit {
        let literal = Lit::positive(self.next_variable);
        self.next_variable = self
            .next_variable
            .checked_add(1)
            .expect("SAT variable index overflow");
        literal
    }

    pub fn new_lit_iter(&mut self, count: usize) -> impl Iterator<Item = Lit> + '_ {
        (0..count).map(|_| self.new_lit())
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

    /// A sequential counter encodes the upper bound without enumerating subsets.
    /// Counter entries only need forward implications: the minimal counter values
    /// satisfy the encoding precisely when the requested bound holds.
    pub fn at_most(&mut self, literals: &[Lit], capacity: usize) -> anyhow::Result<()> {
        if capacity >= literals.len() {
            return Ok(());
        }
        if capacity == 0 {
            for &literal in literals {
                self.add_clause(&[!literal])?;
            }
            return Ok(());
        }
        let mut previous: Vec<Lit> = Vec::new();
        for &literal in literals {
            if previous.len() == capacity {
                self.add_clause(&[!literal, !previous[capacity - 1]])?;
            }
            let current: Vec<_> = self
                .new_lit_iter((previous.len() + 1).min(capacity))
                .collect();
            self.add_clause(&[!literal, current[0]])?;
            for (count, &counter) in previous.iter().enumerate() {
                self.add_clause(&[!counter, current[count]])?;
                if count + 1 < current.len() {
                    self.add_clause(&[!literal, !counter, current[count + 1]])?;
                }
            }
            previous = current;
        }
        Ok(())
    }

    pub fn check(&self) -> Result<(), Cancelled> {
        self.cancellation.check()
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
                solver
                    .add_clause(
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
    fn sequential_counter_accepts_exactly_the_assignments_within_the_bound() {
        for size in 0..=6 {
            for capacity in 0..=size {
                let cancellation = crate::computation::Cancellation::default();
                let mut solver =
                    Formula::new(crate::algorithms::sat::backend(&cancellation), cancellation);
                let literals: Vec<_> = solver.new_lit_iter(size).collect();
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
}
