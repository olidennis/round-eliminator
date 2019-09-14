use crate::problem::Problem;
use crate::auto::Auto;
use crate::auto::Sequence;
use crate::auto::Step;
use std::collections::HashMap;

#[derive(Copy,Clone)]
pub struct AutoLb;
impl Auto for AutoLb{
    type Simplification = (usize,usize);

    /// The possible simplifications are given by following the arrows of the diagram.
    fn simplifications(sol : &mut Sequence<Self>, _ : usize) -> Box<dyn Iterator<Item=Self::Simplification>> {
        sol.current_mut().compute_diagram_edges();
        let simpl = sol.current().diagram.as_ref().unwrap().clone();
        Box::new(simpl.into_iter())
    }

    /// Here simplifying means replacing label A with label B, where in the diagram there is an arrow from A to B.
    fn simplify(p : &mut Problem, (c1,c2) : Self::Simplification) -> Option<Problem> {
        // TODO: maybe it makes sense to give none if the problem got trivial (this would require to change should_yield)
        Some(p.replace(c1,c2))
    }

    /// A solution is better if we did more speedup steps to get a trivial problem, or the same but the current one is not a trivial problem.
    /// Also, we yield solutions only if either we reached a trivial problem (so we have a full solution), o we reached the limit.
    fn should_yield(sol : &mut Sequence<Self>, best : &mut Sequence<Self>, maxiter : usize) -> bool {
        let sol_is_trivial = sol.current().is_trivial();
        let best_is_trivial = best.current().is_trivial();

        let better = sol.speedups > best.speedups || ( sol.speedups == best.speedups && !sol_is_trivial && best_is_trivial );
        let end_reached = sol.speedups == maxiter || sol_is_trivial;

        better && end_reached
    }

    /// We should continue trying if we did not reach the speedup steps limit, and
    /// the current solution is still not 0 rounds solvable.
    fn should_continue(sol : &mut Sequence<Self>, _ : &mut Sequence<Self>, maxiter : usize) -> bool {
        let sol_is_trivial = sol.current().is_trivial();

        sol.speedups < maxiter && !sol_is_trivial 
    }

}


impl std::fmt::Display for Sequence<AutoLb> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut cloned = self.clone();
        writeln!(f,"\nLower bound of {} rounds.\n",self.speedups)?;

        let mut lastmap : Option<HashMap<_,_>> = None;

        for step in cloned.steps.iter_mut() {
            let p = match step {
                Step::Initial(p) => {
                    writeln!(f,"\nInitial problem\n{}\n",p.as_result())?;
                    p
                }
                Step::Simplify(((x,y),p)) => {
                    let map = lastmap.unwrap();
                    writeln!(f,"Relax {} -> {}\n",map[x],map[y])?;
                    writeln!(f,"{}\n",p.as_result())?;
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