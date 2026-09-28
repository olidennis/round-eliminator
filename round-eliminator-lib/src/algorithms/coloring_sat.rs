//! Incremental SAT primitives shared by the coloring searches.

#[cfg(feature = "all")]
use rustsat::{
    solvers::{Solve, SolveIncremental, SolverResult},
    types::{Lit, TernaryVal},
};
#[cfg(feature = "all")]
use rustsat_minisat::core::Minisat;
#[cfg(all(not(feature = "all"), feature = "onlyrust"))]
use varisat::{solver::Solver as Varisat, ExtendFormula, Lit as VarisatLit};

/// Literals use signed, one-based numbers. The solver's representation stays
/// inside this adapter, so both native and WebAssembly builds use the same CNF.
pub(super) struct SatEngine {
    variables: usize,
    #[cfg(feature = "all")]
    solver: Minisat,
    #[cfg(all(not(feature = "all"), feature = "onlyrust"))]
    solver: Varisat<'static>,
}

impl SatEngine {
    pub(super) fn new() -> Self {
        Self {
            variables: 0,
            #[cfg(feature = "all")]
            solver: Minisat::default(),
            #[cfg(all(not(feature = "all"), feature = "onlyrust"))]
            solver: Varisat::new(),
        }
    }

    pub(super) fn variable(&mut self) -> i32 {
        self.variables += 1;
        i32::try_from(self.variables).expect("SAT variable space exhausted")
    }

    pub(super) fn clause(
        &mut self,
        literals: impl IntoIterator<Item = i32>,
    ) -> Result<(), &'static str> {
        let literals: Vec<_> = literals.into_iter().collect();
        #[cfg(feature = "all")]
        self.solver
            .add_clause(
                literals
                    .into_iter()
                    .map(|lit| Lit::new(lit.unsigned_abs() - 1, lit < 0))
                    .collect(),
            )
            .map_err(|_| "Could not add coloring SAT clause")?;
        #[cfg(all(not(feature = "all"), feature = "onlyrust"))]
        self.solver.add_clause(
            &literals
                .into_iter()
                .map(|lit| VarisatLit::from_dimacs(lit as isize))
                .collect::<Vec<_>>(),
        );
        Ok(())
    }

    pub(super) fn solve(&mut self) -> Result<Option<Vec<bool>>, &'static str> {
        self.solve_assuming(&[])
    }

    pub(super) fn solve_assuming(
        &mut self,
        assumptions: &[i32],
    ) -> Result<Option<Vec<bool>>, &'static str> {
        #[cfg(feature = "all")]
        {
            let assumptions: Vec<_> = assumptions
                .iter()
                .map(|&lit| Lit::new(lit.unsigned_abs() - 1, lit < 0))
                .collect();
            match self
                .solver
                .solve_assumps(&assumptions)
                .map_err(|_| "Coloring SAT solver failed")?
            {
                SolverResult::Unsat => return Ok(None),
                SolverResult::Sat => {
                    let solution = self
                        .solver
                        .full_solution()
                        .map_err(|_| "Missing SAT model")?;
                    return Ok(Some(
                        (0..self.variables)
                            .map(|i| {
                                solution.lit_value(Lit::new(i as u32, false)) == TernaryVal::True
                            })
                            .collect(),
                    ));
                }
                _ => return Err("Coloring SAT solver returned an inconclusive result"),
            }
        }
        #[cfg(all(not(feature = "all"), feature = "onlyrust"))]
        {
            self.solver.assume(
                &assumptions
                    .iter()
                    .map(|&lit| VarisatLit::from_dimacs(lit as isize))
                    .collect::<Vec<_>>(),
            );
            if !self
                .solver
                .solve()
                .map_err(|_| "Coloring SAT solver failed")?
            {
                return Ok(None);
            }
            let mut model = vec![false; self.variables];
            for lit in self.solver.model().ok_or("Missing SAT model")? {
                let index = lit.to_dimacs().unsigned_abs() - 1;
                model[index] = lit.is_positive();
            }
            Ok(Some(model))
        }
        #[cfg(not(any(feature = "all", feature = "onlyrust")))]
        {
            let _ = assumptions;
            Err("Coloring solvability requires a SAT backend")
        }
    }
}

/// A bitonic cardinality network. `result[i]` is true exactly when at least
/// `i + 1` of the input literals are true.
pub(super) fn cardinality_network(
    sat: &mut SatEngine,
    inputs: &[i32],
) -> Result<Vec<i32>, &'static str> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let false_lit = sat.variable();
    sat.clause([-false_lit])?;
    let mut values = inputs.to_vec();
    values.resize(inputs.len().next_power_of_two(), false_lit);
    fn compare(sat: &mut SatEngine, a: i32, b: i32) -> Result<(i32, i32), &'static str> {
        let high = sat.variable();
        let low = sat.variable();
        sat.clause([-a, high])?;
        sat.clause([-b, high])?;
        sat.clause([a, b, -high])?;
        sat.clause([a, -low])?;
        sat.clause([b, -low])?;
        sat.clause([-a, -b, low])?;
        Ok((high, low))
    }
    fn merge(
        sat: &mut SatEngine,
        values: &mut [i32],
        descending: bool,
    ) -> Result<(), &'static str> {
        if values.len() <= 1 {
            return Ok(());
        }
        let half = values.len() / 2;
        for i in 0..half {
            let (high, low) = compare(sat, values[i], values[i + half])?;
            (values[i], values[i + half]) = if descending { (high, low) } else { (low, high) };
        }
        merge(sat, &mut values[..half], descending)?;
        merge(sat, &mut values[half..], descending)
    }
    fn sort(sat: &mut SatEngine, values: &mut [i32], descending: bool) -> Result<(), &'static str> {
        if values.len() <= 1 {
            return Ok(());
        }
        let half = values.len() / 2;
        sort(sat, &mut values[..half], true)?;
        sort(sat, &mut values[half..], false)?;
        merge(sat, values, descending)
    }
    sort(sat, &mut values, true)?;
    values.truncate(inputs.len());
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::{cardinality_network, SatEngine};

    #[test]
    fn cardinality_network_matches_every_small_assignment() {
        for count in 1..=5 {
            let mut sat = SatEngine::new();
            let inputs: Vec<_> = (0..count).map(|_| sat.variable()).collect();
            let thresholds = cardinality_network(&mut sat, &inputs).unwrap();
            for bits in 0usize..(1 << count) {
                let fixed: Vec<_> = inputs
                    .iter()
                    .enumerate()
                    .map(|(i, &lit)| if bits & (1 << i) == 0 { -lit } else { lit })
                    .collect();
                for (index, &threshold) in thresholds.iter().enumerate() {
                    let mut assumptions = fixed.clone();
                    assumptions.push(threshold);
                    assert_eq!(
                        sat.solve_assuming(&assumptions).unwrap().is_some(),
                        bits.count_ones() as usize >= index + 1,
                        "count={count}, bits={bits}, threshold={}",
                        index + 1,
                    );
                }
            }
        }
    }
}
