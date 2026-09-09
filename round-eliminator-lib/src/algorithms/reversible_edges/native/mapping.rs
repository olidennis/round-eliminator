//! Context/occurrence-dependent local mappings. Edge clauses quantify over
//! every possible pair of endpoint contexts, not merely one pairing of stars.
use super::*;
use itertools::Itertools;
use rustsat::{
    solvers::{Interrupt, InterruptSolver, Solve, SolverResult},
    types::{Lit, TernaryVal},
};
use rustsat_minisat::{core::Minisat, Limit};
use std::sync::mpsc;

struct Cnf<'a, 'b> {
    solver: Minisat,
    next: usize,
    clauses: usize,
    budget: &'a Budget<'b>,
}
impl Cnf<'_, '_> {
    fn var(&mut self) -> Result<Lit, String> {
        if self.next >= self.budget.options.max_variables {
            return Err(LIMIT.into());
        }
        let lit = Lit::new(self.next as u32, false);
        self.next += 1;
        Ok(lit)
    }
    fn clause(
        &mut self,
        lits: impl IntoIterator<Item = Lit>,
        eh: &EventHandler,
    ) -> Result<(), String> {
        self.budget.check(eh)?;
        self.clauses += 1;
        if self.clauses > 2_000_000 {
            return Err(LIMIT.into());
        }
        self.solver
            .add_clause(lits.into_iter().collect())
            .map_err(|e| e.to_string())
    }
    fn one(&mut self, lits: &[Lit], eh: &EventHandler) -> Result<(), String> {
        self.clause(lits.iter().copied(), eh)?;
        for i in 0..lits.len() {
            for j in 0..i {
                self.clause([!lits[i], !lits[j]], eh)?;
            }
        }
        Ok(())
    }
}

pub(super) fn find(
    input: &Input,
    target: &Input,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Option<Vec<Vec<Label>>>, String> {
    Ok(find_guarded(input, target, &BTreeMap::new(), 0, budget, eh)?.map(|(outputs, _)| outputs))
}

pub(super) fn find_guarded(
    input: &Input,
    target: &Input,
    guards: &BTreeMap<[usize; 2], Vec<(usize, bool)>>,
    parameters: usize,
    budget: &Budget,
    eh: &mut EventHandler,
) -> Result<Option<(Vec<Vec<Label>>, Vec<bool>)>, String> {
    let mut ordered = BTreeSet::new();
    for row in &target.nodes {
        for perm in row.iter().copied().permutations(row.len()) {
            budget.check(eh)?;
            ordered.insert(perm);
            if ordered.len() > budget.options.max_configurations {
                return Err(LIMIT.into());
            }
        }
    }
    let mut cnf = Cnf {
        solver: Minisat::default(),
        next: 0,
        clauses: 0,
        budget,
    };
    let labels = target.names.len();
    let parameters = (0..parameters)
        .map(|_| cnf.var())
        .collect::<Result<Vec<_>, _>>()?;
    let mut support = vec![];
    for _ in &input.names {
        support.push(
            (0..labels)
                .map(|_| cnf.var())
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let mut outputs = vec![];
    for row in &input.nodes {
        budget.check(eh)?;
        let mut ports = vec![];
        for &state in row {
            let x = (0..labels)
                .map(|_| cnf.var())
                .collect::<Result<Vec<_>, _>>()?;
            cnf.one(&x, eh)?;
            for a in 0..labels {
                cnf.clause([!x[a], support[state][a]], eh)?;
            }
            ports.push(x);
        }
        let mut alternatives = vec![];
        for legal in &ordered {
            if legal.len() != row.len() {
                return Err("Mapping node degrees differ".into());
            }
            let selected = cnf.var()?;
            alternatives.push(selected);
            for (i, &a) in legal.iter().enumerate() {
                cnf.clause([!selected, ports[i][a]], eh)?;
            }
        }
        cnf.clause(alternatives, eh)?;
        outputs.push(ports);
    }
    for &[s, t] in &input.edges {
        for a in 0..labels {
            for b in 0..labels {
                if !target.edges.contains(&annotations::edge(a, b)) {
                    let mut clause = vec![!support[s][a], !support[t][b]];
                    if let Some(conditions) = guards.get(&annotations::edge(s, t)) {
                        for &(k, positive) in conditions {
                            clause.push(if positive {
                                !parameters[k]
                            } else {
                                parameters[k]
                            });
                        }
                    }
                    cnf.clause(clause, eh)?;
                }
            }
        }
    }
    eh.notify("Reversible edges: SAT mapping variables", cnf.next, 0);
    cnf.solver.set_limit(Limit::Conflicts(50_000));
    // Keep the solver alive outside the scope. On STOP/unwind the guard
    // interrupts before scope joins; the interrupter can never outlive it.
    let interrupter = cnf.solver.interrupter();
    let result = std::thread::scope(|scope| -> Result<SolverResult, String> {
        struct Stop<'a>(&'a rustsat_minisat::core::Interrupter);
        impl Drop for Stop<'_> {
            fn drop(&mut self) {
                self.0.interrupt();
            }
        }
        let _stop = Stop(&interrupter);
        let (tx, rx) = mpsc::channel();
        let solver = &mut cnf.solver;
        scope.spawn(move || {
            let _ = tx.send(solver.solve().map_err(|e| e.to_string()));
        });
        loop {
            budget.check(eh)?;
            eh.notify(
                "Reversible edges: solving reverse mapping",
                input.nodes.len(),
                0,
            );
            match rx.recv_timeout(Duration::from_millis(30)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("Reverse-mapping SAT worker disconnected".into()),
            }
        }
    })?;
    match result {
        SolverResult::Unsat => return Ok(None),
        SolverResult::Interrupted => return Err(LIMIT.into()),
        SolverResult::Sat => {}
    }
    let model = cnf.solver.full_solution().map_err(|e| e.to_string())?;
    let mut mapping = vec![];
    for row in outputs {
        let mut mapped = vec![];
        for port in row {
            let values: Vec<_> = port
                .iter()
                .enumerate()
                .filter(|(_, l)| model.lit_value(**l) == TernaryVal::True)
                .collect();
            if values.len() != 1 {
                return Err("Invalid SAT mapping assignment".into());
            }
            mapped.push(target.base[values[0].0]);
        }
        mapping.push(mapped);
    }
    let chosen = parameters
        .iter()
        .map(|&l| model.lit_value(l) == TernaryVal::True)
        .collect();
    Ok(Some((mapping, chosen)))
}

pub(super) fn verify(
    input: &Input,
    target: &Input,
    outputs: &[Vec<Label>],
    budget: &Budget,
    eh: &EventHandler,
) -> Result<(), String> {
    if outputs.len() != input.nodes.len() {
        return Err("Missing mapping contexts".into());
    }
    let target_nodes: BTreeSet<Vec<_>> = target
        .nodes
        .iter()
        .map(|r| {
            let mut r: Vec<_> = r.iter().map(|&s| target.base[s]).collect();
            r.sort();
            r
        })
        .collect();
    let target_edges: BTreeSet<_> = target
        .edges
        .iter()
        .map(|&[a, b]| pair(target.base[a], target.base[b]))
        .collect();
    let mut support = vec![BTreeSet::new(); input.names.len()];
    for (row, mapped) in input.nodes.iter().zip(outputs) {
        budget.check(eh)?;
        let mut sorted = mapped.clone();
        sorted.sort();
        if row.len() != mapped.len() || !target_nodes.contains(&sorted) {
            return Err("Reverse mapping violates an original node configuration".into());
        }
        for (&s, &a) in row.iter().zip(mapped) {
            support[s].insert(a);
        }
    }
    for &[s, t] in &input.edges {
        budget.check(eh)?;
        for &a in &support[s] {
            for &b in &support[t] {
                if !target_edges.contains(&pair(a, b)) {
                    return Err("Reverse mapping violates an original edge configuration".into());
                }
            }
        }
    }
    Ok(())
}
