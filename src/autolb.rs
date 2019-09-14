use crate::problem::Problem;
use std::collections::HashMap;

#[derive(Clone)]
enum Step{
    Initial(Problem),
    Relax(((usize,usize),Problem)),
    Speedup(Problem)
}

#[derive(Clone)]
struct Sequence {
    steps : Vec<Step>,
    speedups : usize,
}

impl Sequence {
	fn new(p : Problem) -> Self {
		Self{ steps : vec![Step::Initial(p)], speedups : 0 }
	}

    fn current(&self) -> &Problem {
        match self.steps.last().unwrap() {
            Step::Initial(p) => {p},
            Step::Relax((_,p)) => {p},
            Step::Speedup(p) => {p}
        }
    }

    fn current_mut(&mut self) -> &mut Problem {
        match self.steps.last_mut().unwrap() {
            Step::Initial(p) => {p},
            Step::Relax((_,p)) => {p},
            Step::Speedup(p) => {p}
        }
    }
	
	fn better_than(self : &mut Sequence, other : &mut Sequence) -> bool {
        if self.speedups > other.speedups {
            return true;
        }
        self.current_mut().compute_triviality();
        other.current_mut().compute_triviality();
		( self.speedups == other.speedups && !self.current().is_trivial.unwrap() && other.current().is_trivial.unwrap() )
	}

    fn make_printable(&mut self) {
        for step in self.steps.iter_mut() {
            match step {
                Step::Initial(p) => {let _ = p.as_result(); },
                Step::Relax((_,p)) => {let _ = p.as_result(); },
                Step::Speedup(p) => {let _ = p.as_result(); }
            }
        }
    }

    fn push(&mut self, step : Step){
        self.steps.push(step);
    }

    fn pop(&mut self){
        self.steps.pop();
    }

    fn pop_speedup(&mut self){
        self.speedups -= 1;
        self.pop();
    }

    fn push_speedup(&mut self) {
        self.speedups += 1;
        let last = self.current_mut();
        let new = last.speedup();
        self.push(Step::Speedup(new));
    }

    fn push_relax(&mut self, c1 : usize, c2 : usize) {
        let last = self.current_mut();
        let new = last.replace(c1,c2);
        self.push(Step::Relax(((c1,c2),new)));
    }

    fn pop_relax(&mut self){
        self.pop();
    }

}

impl std::fmt::Display for Sequence {
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
                Step::Relax(((x,y),p)) => {
                    let map = lastmap.unwrap();
                    writeln!(f,"Relax {} -> {}",map[x],map[y])?;
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

pub fn autolb(name : &str, labels : usize, iter : usize){
    let data = std::fs::read_to_string(name).expect("Unable to read file");
    
    let p = Problem::from_line_separated_text(&data);
	let mut sol = Sequence::new(p);
	let mut best = sol.clone();
	
	problem(&mut sol, &mut best, labels, iter);	
}

fn problem(sol : &mut Sequence, best : &mut Sequence, maxlabels : usize, maxiter : usize) {
	
	if sol.better_than(best) && (sol.speedups == maxiter || sol.current().is_trivial.unwrap() ){
		*best = sol.clone();
        best.make_printable();
		println!("\n\n\n\n{}\n\n\n\n",best);
	}
	
	if sol.speedups < maxiter && !sol.current().is_trivial.unwrap() {
		simplify(sol, best, maxlabels, maxiter );
	}
	
}


fn simplify(sol : &mut Sequence, best : &mut Sequence, maxlabels : usize, maxiter : usize) {
	
    if sol.current().num_labels() <= maxlabels {
        sol.push_speedup();
        problem(sol,best,maxlabels,maxiter);
        sol.pop_speedup();
    } else {
        sol.current_mut().compute_diagram_edges();
        let simpl = sol.current().diagram.as_ref().unwrap().clone();
	
		for &(c1,c2) in simpl.iter() {
            sol.push_relax(c1,c2);
			simplify(sol,best,maxlabels,maxiter);
            sol.pop_relax();
		}
		
	}
	
}
