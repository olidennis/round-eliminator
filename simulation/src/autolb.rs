use crate::auto::Auto;
use crate::auto::Sequence;
use crate::auto::Step;
use crate::problem::Problem;
use std::collections::HashMap;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AutoLb {
    done: HashSet<(usize, Problem)>,
}

impl Auto for AutoLb {
    type Simplification = (usize, usize);

    fn new() -> Self {
        Self {
            done: HashSet::new(),
        }
    }

    /// The possible simplifications are given by following the arrows of the diagram.
    fn simplifications(
        &mut self,
        sol: &mut Sequence<Self>,
        _: usize,
    ) -> Box<dyn Iterator<Item = Self::Simplification>> {
        let simpl = sol.current().diagram.clone();
        Box::new(simpl.into_iter())
    }

    /// Here simplifying means replacing label A with label B, where in the diagram there is an arrow from A to B.
    fn simplify(
        &mut self,
        sequence: &mut Sequence<Self>,
        (c1, c2): Self::Simplification,
    ) -> Option<Problem> {
        let speedups = sequence.speedups;
        let p = sequence.current_mut();
        let np = p.replace(c1, c2,false);
        if np.is_trivial || !self.done.insert((speedups, np.clone())) {
            return None;
        }
        Some(np)
    }

    /// A solution is better if we did more speedup steps to get a trivial problem, or the same but the current one is not a trivial problem.
    fn should_yield(
        &mut self,
        sol: &mut Sequence<Self>,
        best: &mut Sequence<Self>,
        _: usize,
    ) -> bool {
        let sol_is_trivial = sol.current().is_trivial;
        let best_is_trivial = best.current().is_trivial;

        let should_yield = sol.speedups > best.speedups
            || (sol.speedups == best.speedups && !sol_is_trivial && best_is_trivial);
        
        if should_yield {
            for x in sol.steps.iter_mut() {
                if let Step::Simplify((_, p)) = x {
                    p.compute_diagram_edges_from_rightconstraints();
                }
            }
        }
        should_yield
    }

    /// We should continue trying if we did not reach the speedup steps limit, and
    /// the current solution is still not 0 rounds solvable.
    fn should_continue(
        &mut self,
        sol: &mut Sequence<Self>,
        _: &mut Sequence<Self>,
        maxiter: usize,
        colors: usize
    ) -> bool {
        let sol_is_trivial = sol.current().is_trivial || sol.current().coloring >= colors;

        sol.speedups < maxiter && !sol_is_trivial
    }
}

impl std::fmt::Display for Sequence<AutoLb> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut cloned = self.clone();
        writeln!(
            f,
            "\nLower bound of {} rounds.\n",
            self.speedups + if self.current().is_trivial { 0 } else { 1 }
        )?;

        let mut lastmap: Option<HashMap<_, _>> = None;

        for step in cloned.steps.iter_mut() {
            let p = match step {
                Step::Initial(p) => {
                    writeln!(f, "\nInitial problem\n{}\n", p.as_result())?;
                    p
                }
                Step::Simplify(((x, y), p)) => {
                    let map = lastmap.unwrap();
                    writeln!(f, "Relax {} -> {}\n", map[x], map[y])?;
                    writeln!(f, "{}\n", p.as_result())?;
                    p
                }
                Step::Speedup(p) => {
                    writeln!(f, "\nSpeed up\n\n{}\n", p.as_result())?;
                    p
                }
            };
            lastmap = Some(p.map_label_text());
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
pub enum ResultStep{
    Initial,
    Simplified(Vec<(String,String)>),
    Speedup
}

pub struct ResultAutoLb{
    pub steps : Vec<(ResultStep,Problem)>
}

impl Sequence<AutoLb> {
    pub fn as_result(&self) -> ResultAutoLb {
        let mut v = vec![];
        let mut simpls = vec![];
        let mut lastp : Option<Problem> = None;
        let mut lastmap: Option<HashMap<usize, String>> = None;

        for step in self.steps.iter() {
            let p = match step {
                Step::Initial(p) => {
                    v.push((ResultStep::Initial,p.clone()));
                    p
                }
                Step::Simplify(((x, y), p)) => {
                    let map = lastmap.unwrap();
                    simpls.push((map[x].clone(), map[y].clone()));
                    p
                }
                Step::Speedup(p) => {
                    if !simpls.is_empty() {
                        v.push((ResultStep::Simplified(simpls.clone()),lastp.take().unwrap()));
                    }
                    simpls = vec![];
                    v.push((ResultStep::Speedup,p.clone()));
                    p
                }
            };
            lastmap = Some(p.map_label_text());
            lastp = Some(p.clone());
        }

        ResultAutoLb{ steps : v }
    }
}