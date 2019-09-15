use crate::problem::Problem;
use crate::auto::Auto;
use crate::auto::Sequence;
use crate::bignum::BigNum;
use crate::auto::Step;
use std::collections::HashMap;

#[derive(Clone)]
pub struct AutoUb;
impl Auto for AutoUb{
    type Simplification = BigNum;


    fn new() -> Self {
        Self
    }
    /// The possible simplifications are described by sets of labels,
    /// the valid ones are the sets containing at most `maxlabel` labels
    fn simplifications(&mut self, sol : &mut Sequence<Self>, maxlabels : usize) -> Box<dyn Iterator<Item=Self::Simplification>> {
        let iter = sol.current().all_possible_sets().filter(move |x|x.count_ones() <= maxlabels as u32);
        Box::new(iter)
    }

    /// Here simplification means making the problem harder,
    /// restricting the label set to the ones contained in `mask`.
    fn simplify(&mut self, sequence : &mut Sequence<Self>, mask : Self::Simplification) -> Option<Problem> {
        let p = sequence.current_mut();
        // Unfortunately, it may happen that if we use set inclusion to compute the diagram, we miss some edges.
        // So we need to use the right constraints.
        // The problem is that, while having more edges may help the speedup,
        // this actually slows down autoub,
        // but we still need to use this slower version, otherwise we would show the wrong diagram to the user
        // so we fix this by using the wrong diagram, that is still correct to use to do speedup, since it only misses edges,
        // and we recompute the correct only when we yield a solution.
        p.harden(mask, false)
    }

    /// A solution is better if the current problem is 0 rounds solvable and
    /// either the other problem is non trivial, or both are trivial and the current one requires less rounds.
    fn should_yield(&mut self, sol : &mut Sequence<Self>, best : &mut Sequence<Self>, _ : usize) -> bool {
        let sol_is_trivial = sol.current().is_trivial();
        let best_is_trivial = best.current().is_trivial();

        let should_yield = sol_is_trivial && ( !best_is_trivial || sol.speedups < best.speedups );
        if should_yield {
            for x in sol.steps.iter_mut() {
                let p = match x {
                    Step::Initial(p) => {p},
                    Step::Simplify((_,p)) => {p},
                    Step::Speedup(p) => {p}
                };
                p.compute_diagram_edges_from_rightconstraints();
            }
        }
        should_yield
    }

    /// We should continue trying if we did not reach the speedup steps limit, and
    /// the current solution is still not 0 rounds solvable, and
    /// either we still have no solutions or we can improve it by at least one round.
    fn should_continue(&mut self, sol : &mut Sequence<Self>, best : &mut Sequence<Self>, maxiter : usize) -> bool {
        let sol_is_trivial = sol.current().is_trivial();
        let best_is_trivial = best.current().is_trivial();

        sol.speedups < maxiter && !sol_is_trivial && (!best_is_trivial || best.speedups -1 > sol.speedups ) 
    }

}


impl std::fmt::Display for Sequence<AutoUb> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut cloned = self.clone();
        writeln!(f,"\nUpper bound of {} rounds.\n",self.speedups)?;

        let mut lastmap : Option<HashMap<_,_>> = None;

        for step in cloned.steps.iter_mut() {
            let p = match step {
                Step::Initial(p) => {
                    writeln!(f,"\nInitial problem\n{}\n",p.as_result())?;
                    p
                }
                Step::Simplify((mask,p)) => {
                    let map = lastmap.unwrap();
                    writeln!(f,"Kept labels")?;
                    for x in mask.one_bits() {
                        writeln!(f,"{}",map[&x])?;
                    }
                    writeln!(f,"\n{}\n",p.as_result())?;
                    p
                },
                Step::Speedup(p) => {
                    writeln!(f,"\nSpeed up\n\n{}\n",p.as_result())?;
                    p
                },
            };
            lastmap = Some(p.map_label_text());
        }
        Ok(())
	}
}