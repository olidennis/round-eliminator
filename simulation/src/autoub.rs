use crate::auto::Auto;
use crate::auto::Sequence;
use crate::auto::Step;
use crate::problem::DiagramType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::bignum::BigNum as _;
use crate::bignum::BigBigNum as BigNum;
type Problem = crate::problem::GenericProblem;


#[derive(Clone)]
pub struct AutoUb {
    usepred: bool,
    det: bool,
}
impl Auto for AutoUb {
    type Simplification = BigNum;

    fn new(features: &[&str]) -> Self {
        Self {
            usepred: features.iter().any(|&x| x == "pred"),
            det: features.iter().any(|&x| x == "det"),
        }
    }
    /// The possible simplifications are described by sets of labels,
    /// the valid ones are the sets containing at most `maxlabel` labels
    fn simplifications(
        &mut self,
        sol: &mut Sequence<Self>,
        maxlabels: usize,
    ) -> Box<dyn Iterator<Item = Self::Simplification>> {
        if !self.det || sol.current().map_label_oldset.is_none() || sol.speedups == 0 {
            let iter = sol
                .current()
                .all_possible_sets()
                .filter(move |x| x.count_ones() == maxlabels as u32);
            Box::new(iter)
        } else {
            let labels: Vec<usize> = sol
                .current()
                .map_text_oldlabel
                .as_ref()
                .unwrap()
                .iter()
                .cloned()
                .map(|(_, x)| x)
                .collect();
            let map = sol.current().map_label_oldset.as_ref().unwrap();
            let mut keep = vec![];
            for lab in labels {
                let mut good: Vec<_> = map
                    .iter()
                    .cloned()
                    .filter(|(_, oldset)| oldset.bit(lab))
                    .collect();
                good.sort_by_key(|(_, oldset)| oldset.count_ones());
                let sz = good[0].1.count_ones();
                let min = good
                    .into_iter()
                    .take_while(|(_, oldset)| oldset.count_ones() == sz);
                keep.extend(min.map(|(lab, _)| lab));
            }
            //println!("KEEPING ONLY {:?}", keep);
            let set = keep
                .iter()
                .fold(BigNum::zero(), |a, &b| a | (BigNum::one() << b));
            let iter = std::iter::once(set);
            Box::new(iter)
        }
    }

    /// Here simplification means making the problem harder,
    /// restricting the label set to the ones contained in `mask`.
    fn simplify(
        &mut self,
        sequence: &mut Sequence<Self>,
        mask: Self::Simplification,
    ) -> Option<Problem> {
        let p = sequence.current_mut();
        // Unfortunately, it may happen that if we use set inclusion to compute the diagram, we miss some edges.
        // So we need to use the right constraints.
        // The problem is that, while having more edges may help the speedup,
        // this actually slows down autoub,
        // but we still need to use this slower version, otherwise we would show the wrong diagram to the user
        // so we fix this by using the wrong diagram, that is still correct to use to do speedup, since it only misses edges,
        // and we recompute the correct only when we yield a solution.
        p.harden(mask, DiagramType::Accurate, self.usepred)
    }

    /// A solution is better if the current problem is 0 rounds solvable and
    /// either the other problem is non trivial, or both are trivial and the current one requires less rounds.
    fn should_yield(
        &mut self,
        sol: &mut Sequence<Self>,
        best: &mut Sequence<Self>,
        _: usize
    ) -> bool {
        let sol_is_trivial = sol.current().is_zero_rounds();
        let best_is_trivial = best.current().is_zero_rounds();

        let should_yield = sol_is_trivial && (!best_is_trivial || sol.speedups < best.speedups);
        /*
        if should_yield {
            for x in sol.steps.iter_mut() {
                if let Step::Simplify((_, p)) = x {
                    p.compute_diagram_edges_from_rightconstraints();
                }
            }
        }*/
        should_yield
    }

    /// We should continue trying if we did not reach the speedup steps limit, and
    /// the current solution is still not 0 rounds solvable, and
    /// either we still have no solutions or we can improve it by at least one round.
    /// If the current problem is a fixpoint, we stop
    fn should_continue(
        &mut self,
        sol: &mut Sequence<Self>,
        best: &mut Sequence<Self>,
        maxiter: usize
    ) -> bool {
        if let Some((i, Step::Speedup(p))) = sol.steps.iter().enumerate().last() {
            let t = p.as_result().to_string();
            for (j, x) in sol.steps.iter().enumerate() {
                if i == j || j + 4 < i {
                    continue;
                }
                if let Step::Speedup(x) = x {
                    let t2 = x.as_result().to_string();
                    if t == t2 {
                        return false;
                    }
                }
            }
        }

        let sol_is_trivial = sol.current().is_zero_rounds();
        let best_is_trivial = best.current().is_zero_rounds();

        sol.speedups < maxiter
            && !sol_is_trivial
            && (!best_is_trivial || best.speedups - 1 > sol.speedups)
    }
}

impl std::fmt::Display for Sequence<AutoUb> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut cloned = self.clone();
        writeln!(f, "\nUpper bound of {} rounds.\n", self.speedups)?;

        let mut lastmap: Option<HashMap<_, _>> = None;

        for step in cloned.steps.iter_mut() {
            let p = match step {
                Step::Initial(p) => {
                    writeln!(f, "\nInitial problem\n{}\n", p.as_result())?;
                    p
                }
                Step::Simplify((mask, p)) => {
                    let map = lastmap.unwrap();
                    writeln!(f, "Kept labels")?;
                    for x in mask.one_bits() {
                        writeln!(f, "{}", map[&x])?;
                    }
                    writeln!(f, "\n{}\n", p.as_result())?;
                    p
                }
                Step::Speedup(p) => {
                    writeln!(f, "\nSpeed up\n\n{}\n", p.as_result())?;
                    p
                }
                Step::MergeEqual(p) => {
                    writeln!(f, "\nMerged equal labels\n\n{}\n", p.as_result())?;
                    p
                }
            };
            lastmap = Some(p.map_label_text());
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
pub enum ResultStep {
    Initial,
    Simplified(Vec<String>),
    Speedup,
    MergedEqual
}

pub struct ResultAutoUb {
    pub steps: Vec<(ResultStep, Problem)>,
}

impl Sequence<AutoUb> {
    pub fn as_result(&self) -> ResultAutoUb {
        let mut v = vec![];
        let mut lastmap: Option<HashMap<usize, String>> = None;

        for step in self.steps.iter() {
            let p = match step {
                Step::Initial(p) => {
                    v.push((ResultStep::Initial, p.clone()));
                    p
                }
                Step::Simplify((mask, p)) => {
                    let map = lastmap.unwrap();
                    let simpls = mask.one_bits().map(|x| map[&x].to_owned()).collect();
                    v.push((ResultStep::Simplified(simpls), p.clone()));
                    p
                }
                Step::Speedup(p) => {
                    v.push((ResultStep::Speedup, p.clone()));
                    p
                }
                Step::MergeEqual(p) => {
                    v.push((ResultStep::MergedEqual, p.clone()));
                    p
                }
            };
            lastmap = Some(p.map_label_text());
        }

        ResultAutoUb { steps: v }
    }
}
