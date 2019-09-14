use crate::problem::Problem;

#[derive(Clone)]
pub enum Step<T:Clone>{
    Initial(Problem),
    Simplify((T,Problem)),
    Speedup(Problem)
}

pub trait Auto : Sized + Copy + Clone{
    type Simplification : Copy + Clone ;
    fn simplifications(sequence : &mut Sequence<Self>, maxlabels : usize) -> Box<dyn Iterator<Item=Self::Simplification>>;
    fn should_yield(sequence : &mut Sequence<Self>, best : &mut Sequence<Self>, maxiter : usize) -> bool;
    fn should_continue(sequence : &mut Sequence<Self>, best : &mut Sequence<Self>, maxiter : usize) -> bool;
    fn simplify(p : &mut Problem, simpl : Self::Simplification) -> Problem;
}

#[derive(Clone)]
pub struct Sequence<T> where T : Auto {
    pub steps : Vec<Step<T::Simplification>>,
    pub speedups : usize,
}

impl<T> Sequence<T> where T : Auto {
    pub fn new(p : Problem) -> Self {
		Self{ steps : vec![Step::Initial(p)], speedups : 0 }
	}

    pub fn current(&self) -> &Problem {
        match self.steps.last().unwrap() {
            Step::Initial(p) => {p},
            Step::Simplify((_,p)) => {p},
            Step::Speedup(p) => {p}
        }
    }

    pub fn current_mut(&mut self) -> &mut Problem {
        match self.steps.last_mut().unwrap() {
            Step::Initial(p) => {p},
            Step::Simplify((_,p)) => {p},
            Step::Speedup(p) => {p}
        }
    }
	

    fn make_printable(&mut self) {
        for step in self.steps.iter_mut() {
            match step {
                Step::Initial(p) => {let _ = p.as_result(); },
                Step::Simplify((_,p)) => {let _ = p.as_result(); },
                Step::Speedup(p) => {let _ = p.as_result(); }
            }
        }
    }

    fn push(&mut self, step : Step<T::Simplification>){
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

    fn push_simplification(&mut self, simpl : T::Simplification ) {
        let last = self.current_mut();
        let new = T::simplify(last,simpl);
        self.push(Step::Simplify((simpl,new)));
    }

    fn pop_simplification(&mut self){
        self.pop();
    }
}

pub struct AutomaticSimplifications<T : Auto> {
    pub sol : Sequence<T>,
    pub best : Sequence<T>,
    pub maxiter : usize,
    pub maxlabels : usize
}

impl<T:Auto> AutomaticSimplifications<T> {
    pub fn new(p : Problem, maxiter : usize, maxlabels : usize) -> Self {

        let sol = Sequence::new(p);
        let best = sol.clone();
        Self { sol, best , maxiter, maxlabels}
    }
    pub fn run<F>(&mut self, mut cb : F) where F : FnMut(&Sequence<T>){
        self.problem(&mut cb);	
    }

    fn problem<F>(&mut self, cb : &mut F) where F : FnMut(&Sequence<T>){
        if T::should_yield(&mut self.sol, &mut self.best, self.maxiter) {
            self.best = self.sol.clone();
            self.best.make_printable();
            cb(&self.best);
        }
        if T::should_continue(&mut self.sol, &mut self.best, self.maxiter) {
            self.simplify(cb);
        }
    }
    fn simplify<F>(&mut self, cb : &mut F) where F : FnMut(&Sequence<T>) {
        if self.sol.current().num_labels() <= self.maxlabels {
            self.sol.push_speedup();
            self.problem(cb);
            self.sol.pop_speedup();
        } else {
            for simpl in T::simplifications(&mut self.sol, self.maxlabels) {
                self.sol.push_simplification(simpl);
                self.simplify(cb);
                self.sol.pop_simplification();
            }
            
        }
    }
}


