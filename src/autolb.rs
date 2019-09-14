use crate::problem::Problem;
use crate::auto::Auto;
use crate::auto::Sequence;
use crate::auto::Step;
use std::collections::HashMap;

#[derive(Copy,Clone)]
pub struct AutoLb;
impl Auto for AutoLb{
    type Simplification = (usize,usize);

    fn simplifications(sol : &mut Sequence<Self>, _ : usize) -> Box<dyn Iterator<Item=Self::Simplification>> {
        sol.current_mut().compute_diagram_edges();
        let simpl = sol.current().diagram.as_ref().unwrap().clone();
        Box::new(simpl.into_iter())
    }

    fn simplify(p : &mut Problem, (c1,c2) : Self::Simplification) -> Problem {
        p.replace(c1,c2)
    }

    fn should_yield(sol : &mut Sequence<Self>, best : &mut Sequence<Self>, maxiter : usize) -> bool {
        let sol_is_trivial = sol.current().is_trivial();
        let best_is_trivial = best.current().is_trivial();

        let better = sol.speedups > best.speedups || ( sol.speedups == best.speedups && !sol_is_trivial && best_is_trivial );
        let end_reached = sol.speedups == maxiter || sol_is_trivial;

        better && end_reached
    }

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