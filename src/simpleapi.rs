#![allow(dead_code)]


use crate::problem::Problem;
use crate::problem::ResultProblem;
use crate::autolb::AutoLb;
use crate::autoub::AutoUb;
use crate::auto::AutomaticSimplifications;

pub fn new_problem(left : &str, right : &str) -> (Problem,ResultProblem) {
    let p = Problem::from_text(left, right);
    let r = p.as_result();
    (p,r)
}

pub fn speedup(p : &Problem) -> (Problem,ResultProblem) {
    let np = p.speedup();
    let nr = np.as_result();
    (np,nr)
}

pub fn possible_simplifications(p : &Problem) -> Vec<((usize,usize),(String,String))>{
    let r = p.as_result();
    let pdiag = p.diagram.iter().cloned();
    let rdiag = r.diagram.iter().cloned();
    pdiag.zip(rdiag).collect()
}

pub fn simplify(p : &Problem, (a,b) : (usize,usize)) -> (Problem,ResultProblem)  {
    let np = p.replace(a,b);
    let nr = np.as_result();
    (np,nr)
}

pub fn autolb(p : &Problem, maxiter : usize, maxlabels : usize) -> impl Iterator<Item=Vec<(crate::autolb::ResultStep,Problem,ResultProblem)>>{
    let auto = AutomaticSimplifications::<AutoLb>::new(p.clone(), maxiter, maxlabels);
    auto.into_iter().map(|seq|{
        seq.as_result().steps.into_iter().map(|s|{
            let r = s.1.as_result();
            (s.0,s.1,r)
        }).collect()
    })
}

pub fn autoub(p : &Problem, maxiter : usize, maxlabels : usize) -> impl Iterator<Item=Vec<(crate::autoub::ResultStep,Problem,ResultProblem)>>{
    let auto = AutomaticSimplifications::<AutoUb>::new(p.clone(), maxiter, maxlabels);
    auto.into_iter().map(|seq|{
        seq.as_result().steps.into_iter().map(|s|{
            let r = s.1.as_result();
            (s.0,s.1,r)
        }).collect()
    })
}