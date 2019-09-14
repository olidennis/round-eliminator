use crate::problem::Problem;
use crate::auto::Auto;
use crate::auto::Sequence;
use crate::bignum::BigNum;
use crate::auto::Step;
use std::collections::HashMap;

#[derive(Copy,Clone)]
pub struct AutoUb;
impl Auto for AutoUb{
    type Simplification = BigNum;

    fn simplifications(sol : &mut Sequence<Self>, maxlabels : usize) -> Box<dyn Iterator<Item=Self::Simplification>> {
        let iter = sol.current().all_possible_sets().filter(move |x|x.count_ones() <= maxlabels as u32);
        Box::new(iter)
    }

    fn simplify(p : &mut Problem, mask : Self::Simplification) -> Problem {
        p.harden(mask)
    }

    fn should_yield(sol : &mut Sequence<Self>, best : &mut Sequence<Self>, _ : usize) -> bool {
        let sol_is_trivial = sol.current().is_trivial();
        let best_is_trivial = best.current().is_trivial();

        sol_is_trivial && ( !best_is_trivial || sol.speedups < best.speedups )        
    }

    fn should_continue(sol : &mut Sequence<Self>, best : &mut Sequence<Self>, maxiter : usize) -> bool {
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